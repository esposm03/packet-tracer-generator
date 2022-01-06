use std::{collections::HashMap, fmt::Write, net::IpAddr};

use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use slotmap::{DefaultKey, SlotMap};

/// A router
#[derive(Default, Debug, PartialEq)]
pub struct Device {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub redistribute_ospf_to_rip: bool,
}

/// A link between routers.
///
/// `r1` must always be less than `r2`
#[derive(Default)]
pub struct Link {
    r1: IpNet,
    r2: IpNet,
    ospf_area: Option<u16>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DirectedLink {
    close_key: DefaultKey,
    far_key: DefaultKey,
    close_ip: IpNet,
    far_ip: IpNet,
    ospf_area: Option<u16>,
}

#[derive(Default)]
pub struct App {
    pub devices: SlotMap<DefaultKey, Device>,
    pub links: HashMap<(DefaultKey, DefaultKey), Link>,
    pub rip_enabled: Vec<DefaultKey>,
}

impl App {
    /// Create a new `App`, without devices or links
    pub fn new() -> Self {
        App {
            devices: SlotMap::new(),
            links: HashMap::new(),
            rip_enabled: vec![],
        }
    }

    /// Register a `Device`
    pub fn add_device(&mut self, dev: Device) -> DefaultKey {
        self.devices.insert(dev)
    }

    /// Retrieve a `Device` by name
    pub fn get_device<'a>(&'a mut self, name: &str) -> Option<&'a mut Device> {
        for dev in self.devices.values_mut() {
            if dev.name == name {
                return Some(dev);
            }
        }
        None
    }

    pub fn get_directed_link(
        &self,
        close_key: DefaultKey,
        far_key: DefaultKey,
    ) -> Option<DirectedLink> {
        assert_ne!(close_key, far_key);

        // Order `r_close` and `r_far`
        let (r1_close, key) = if close_key < far_key {
            (true, (close_key, far_key))
        } else {
            (false, (far_key, close_key))
        };

        self.links.get(&key).map(|link| DirectedLink {
            close_key,
            far_key,
            close_ip: if r1_close { link.r1 } else { link.r2 },
            far_ip: if r1_close { link.r2 } else { link.r1 },
            ospf_area: link.ospf_area,
        })
    }

    /// Connect two devices by name
    ///
    /// If the two devices already share a link, then it gets updated
    /// to use the new ip. Otherwise, a new link is created
    pub fn link(&mut self, r1: DefaultKey, r2: DefaultKey, ip: IpNet, ospf_area: Option<u16>) {
        assert_ne!(r1, r2);
        assert!(ip.hosts().count() >= 2);

        // Order `r1` and `r2`
        let (r1, r2) = if r1 < r2 { (r1, r2) } else { (r2, r1) };

        let link = self.links.entry((r1, r2)).or_default();
        let mut hosts = ip.hosts();

        link.r1 = to_ipnet(hosts.next().unwrap(), ip.prefix_len());
        link.r2 = to_ipnet(hosts.next().unwrap(), ip.prefix_len());
        link.ospf_area = ospf_area;
    }

    /// Disconnect the two devices if they are connected
    pub fn unlink(&mut self, r1: DefaultKey, r2: DefaultKey) {
        assert_ne!(r1, r2);

        // Order `r1` and `r2`
        let key = if r1 < r2 { (r1, r2) } else { (r2, r1) };

        self.links.remove(&key);
    }

    pub fn to_commands(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();

        for (close_key, device) in &self.devices {
            let mut res = String::from("enable\nconfigure terminal\n\n");

            // Iterator that returns `(far_key, close_ip, far_ip)`
            let directly_connected = self
                .links
                .iter()
                .filter_map(|(&key, _)| {
                    if key.0 == close_key {
                        Some(key.1)
                    } else if key.1 == close_key {
                        Some(key.0)
                    } else {
                        None
                    }
                })
                .map(|far_key| self.get_directed_link(close_key, far_key).unwrap());

            // Network interfaces
            let mut int_num = 0;
            for link in directly_connected.clone() {
                writeln!(
                    res,
                    concat!(
                        "interface GigabitEthernet {}/0\n",
                        "   ip address {} {}\n",
                        "   no shutdown\n",
                        "exit\n",
                    ),
                    int_num,
                    link.close_ip.addr().to_string(),
                    link.close_ip.netmask().to_string(),
                )
                .unwrap();

                int_num += 1;
            }

            // RIP v2
            res.push_str("router rip\n   version 2\n");
            for link in directly_connected.clone() {
                if self.rip_enabled.contains(&link.far_key) {
                    writeln!(res, "   network {}", link.far_ip.network()).unwrap();
                }
            }
            res.push_str("exit\n\n");

            // OSPF
            res.push_str("router ospf 1\n");
            if device.redistribute_ospf_to_rip {
                res.push_str("   redistribute rip subnets\n")
            }
            for link in directly_connected.clone() {
                if let Some(ospf_area) = link.ospf_area {
                    writeln!(
                        res,
                        "   network {} {} area {}",
                        link.far_ip.addr(),
                        link.far_ip.hostmask(),
                        ospf_area,
                    )
                    .unwrap();
                }
            }
            res.push_str("exit\n\n");

            res.push_str("\nexit\ndisable\n");
            map.insert(device.name.clone(), res);
        }

        map
    }
}

/// Convert an `IpAddr` to an `IpNet` with the given prefix length
fn to_ipnet(ip: IpAddr, cidr: u8) -> IpNet {
    match ip {
        IpAddr::V4(ipv4) => IpNet::V4(Ipv4Net::new(ipv4, cidr).unwrap()),
        IpAddr::V6(ipv6) => IpNet::V6(Ipv6Net::new(ipv6, cidr).unwrap()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn add_remove_ip() {
        let mut app = App::new();

        let r1 = app.add_device(Device {
            name: "R1".to_string(),
            ..Default::default()
        });
        let r2 = app.add_device(Device {
            name: "R2".to_string(),
            ..Default::default()
        });

        app.link(r1, r2, IpNet::from_str("10.0.0.0/30").unwrap(), None);
        assert_eq!(
            app.get_directed_link(r1, r2).unwrap().close_ip,
            "10.0.0.1/30".parse().unwrap(),
        );
        assert_eq!(
            app.get_directed_link(r1, r2).unwrap().far_ip,
            "10.0.0.2/30".parse().unwrap(),
        );
        assert_eq!(app.links.len(), 1);

        app.unlink(r1, r2);
        assert_eq!(app.links.len(), 0);
        assert_eq!(app.get_directed_link(r1, r2), None);
    }

    #[test]
    fn modify_link() {
        let mut app = App::new();

        let r1 = app.add_device(Device {
            name: "R1".to_string(),
            ..Default::default()
        });
        let r2 = app.add_device(Device {
            name: "R2".to_string(),
            ..Default::default()
        });

        app.link(r1, r2, IpNet::from_str("10.0.0.0/30").unwrap(), None);
        assert_eq!(
            app.get_directed_link(r1, r2).unwrap().close_ip,
            "10.0.0.1/30".parse().unwrap(),
        );
        assert_eq!(
            app.get_directed_link(r1, r2).unwrap().far_ip,
            "10.0.0.2/30".parse().unwrap(),
        );

        app.link(r2, r1, IpNet::from_str("10.0.0.4/30").unwrap(), None);
        assert_eq!(
            app.get_directed_link(r1, r2).unwrap().close_ip,
            "10.0.0.5/30".parse().unwrap(),
        );
        assert_eq!(
            app.get_directed_link(r1, r2).unwrap().far_ip,
            "10.0.0.6/30".parse().unwrap(),
        );
    }

    // #[test]
    // fn sus() {
    //     let mut app = App::new();

    //     let r1 = app.add_device(Device {
    //         name: "R1".to_string(),
    //         redistribute_ospf_to_rip: true,
    //     });
    //     let r2 = app.add_device(Device {
    //         name: "R2".to_string(),
    //         redistribute_ospf_to_rip: false,
    //     });

    //     app.link(r1, r2, IpNet::from_str("10.0.0.0/30").unwrap(), Some(10));
    //     assert_eq!(
    //         app.get_directed_link(r1, r2).unwrap().close_ip,
    //         "10.0.0.1/30".parse().unwrap(),
    //     );
    //     assert_eq!(
    //         app.get_directed_link(r1, r2).unwrap().far_ip,
    //         "10.0.0.2/30".parse().unwrap(),
    //     );

    //     app.link(r2, r1, IpNet::from_str("10.0.0.4/30").unwrap(), Some(10));
    //     assert_eq!(
    //         app.get_directed_link(r1, r2).unwrap().close_ip,
    //         "10.0.0.5/30".parse().unwrap(),
    //     );
    //     assert_eq!(
    //         app.get_directed_link(r1, r2).unwrap().far_ip,
    //         "10.0.0.6/30".parse().unwrap(),
    //     );

    //     app.rip_enabled.push(r1);
    //     app.rip_enabled.push(r2);

    //     for (router_name, commands) in app.to_commands() {
    //         println!("{}:\n{}", router_name, commands);
    //     }

    //     todo!();
    // }
}
