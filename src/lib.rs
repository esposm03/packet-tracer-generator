use std::{collections::HashMap, fmt::Write, net::IpAddr};

use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use slotmap::{DefaultKey, SlotMap};

/// A router
#[derive(Default, Debug, PartialEq, Eq, Hash)]
pub struct Device {
    name: String,
}

/// A link between routers.
///
/// `r1` must always be less than `r2`
#[derive(Default)]
pub struct Link {
    r1: IpNet,
    r2: IpNet,
}

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

    /// Connect two devices by name
    ///
    /// If the two devices already share a link, then it gets updated
    /// to use the new ip. Otherwise, a new link is created
    pub fn link(&mut self, r1: DefaultKey, r2: DefaultKey, ip: IpNet) {
        assert_ne!(r1, r2);
        assert!(ip.hosts().count() >= 2);

        // Order `r1` and `r2`
        let (r1, r2) = if r1 < r2 { (r1, r2) } else { (r2, r1) };

        let link = self.links.entry((r1, r2)).or_default();
        let mut hosts = ip.hosts();

        link.r1 = to_ipnet(hosts.next().unwrap(), ip.prefix_len());
        link.r2 = to_ipnet(hosts.next().unwrap(), ip.prefix_len());
    }

    /// Disconnect the two devices if they are connected
    pub fn unlink(&mut self, r1: DefaultKey, r2: DefaultKey) {
        assert_ne!(r1, r2);

        // Order `r1` and `r2`
        let key = if r1 < r2 { (r1, r2) } else { (r2, r1) };

        self.links.remove(&key);
    }

    pub fn get_link_close(&mut self, r_close: DefaultKey, r_far: DefaultKey) -> Option<IpNet> {
        assert_ne!(r_close, r_far);

        // Order `r1` and `r2`
        let key = if r_close < r_far {
            (r_close, r_far)
        } else {
            (r_far, r_close)
        };

        match self.links.get(&key) {
            Some(l) if r_close < r_far => Some(l.r1),
            Some(l) if r_close > r_far => Some(l.r2),
            None => None,
            Some(_) => unreachable!(),
        }
    }

    pub fn get_link_far(&mut self, r_close: DefaultKey, r_far: DefaultKey) -> Option<IpNet> {
        assert_ne!(r_close, r_far);

        // Order `r1` and `r2`
        let key = if r_close < r_far {
            (r_close, r_far)
        } else {
            (r_far, r_close)
        };

        match self.links.get(&key) {
            Some(l) if r_close < r_far => Some(l.r2),
            Some(l) if r_close > r_far => Some(l.r1),
            None => None,
            Some(_) => unreachable!(),
        }
    }

    pub fn to_commands(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();

        for (device_key, device) in &self.devices {
            let mut res = String::from("enable\nconfigure terminal\n\n");

            // Find directly connected devices, and print
            // configuration for every interface
            self.links
                .iter()
                .filter_map(|(&key, link)| {
                    if key.0 == device_key {
                        Some((key.1, link.r1, link.r2))
                    } else if key.1 == device_key {
                        Some((key.0, link.r2, link.r1))
                    } else {
                        None
                    }
                })
                .fold(0, |int_num, (_far_key, close_ip, _far_ip)| {
                    writeln!(
                        res,
                        concat!(
                            "interface GigabitEthernet {}/0\n",
                            "   ip address {} {}\n",
                            "   no shutdown\n",
                            "exit\n",
                        ),
                        int_num,
                        close_ip.addr().to_string(),
                        close_ip.netmask().to_string(),
                    )
                    .unwrap();
                    int_num + 1
                });

            res.push_str("router rip\n   version 2\n");

            // Find directly connected devices which are RIP enabled
            let rip_commands = self
                .links
                .iter()
                .filter_map(|(&key, link)| {
                    if key.0 == device_key {
                        Some((key.1, link.r1, link.r2))
                    } else if key.1 == device_key {
                        Some((key.0, link.r2, link.r1))
                    } else {
                        None
                    }
                })
                .filter(|(far_key, _close_ip, _far_ip)| self.rip_enabled.contains(far_key))
                .map(|(_, _, far_ip)| format!("   network {}\n", far_ip.network()))
                .collect::<String>();
            res.push_str(&rip_commands);
            res.push_str("exit\n");

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
        });
        let r2 = app.add_device(Device {
            name: "R2".to_string(),
        });

        app.link(r1, r2, IpNet::from_str("10.0.0.0/30").unwrap());
        assert_eq!(
            app.get_link_close(r1, r2).unwrap(),
            "10.0.0.1/30".parse().unwrap(),
        );
        assert_eq!(
            app.get_link_far(r1, r2).unwrap(),
            "10.0.0.2/30".parse().unwrap(),
        );
        assert_eq!(app.links.len(), 1);

        app.unlink(r1, r2);
        assert_eq!(app.links.len(), 0);
        assert_eq!(app.get_link_close(r1, r2), None);
        assert_eq!(app.get_link_close(r2, r1), None);
    }

    #[test]
    fn modify_link() {
        let mut app = App::new();

        let r1 = app.add_device(Device {
            name: "R1".to_string(),
        });
        let r2 = app.add_device(Device {
            name: "R2".to_string(),
        });

        app.link(r1, r2, IpNet::from_str("10.0.0.0/30").unwrap());
        assert_eq!(
            app.get_link_close(r1, r2).unwrap(),
            "10.0.0.1/30".parse().unwrap(),
        );
        assert_eq!(
            app.get_link_far(r1, r2).unwrap(),
            "10.0.0.2/30".parse().unwrap(),
        );

        app.link(r2, r1, IpNet::from_str("10.0.0.4/30").unwrap());
        assert_eq!(
            app.get_link_close(r1, r2).unwrap(),
            "10.0.0.5/30".parse().unwrap(),
        );
        assert_eq!(
            app.get_link_far(r1, r2).unwrap(),
            "10.0.0.6/30".parse().unwrap(),
        );
    }

    #[test]
    fn sus() {
        let mut app = App::new();

        let r1 = app.add_device(Device {
            name: "R1".to_string(),
        });
        let r2 = app.add_device(Device {
            name: "R2".to_string(),
        });

        app.link(r1, r2, IpNet::from_str("10.0.0.0/30").unwrap());
        assert_eq!(
            app.get_link_close(r1, r2).unwrap(),
            "10.0.0.1/30".parse().unwrap(),
        );
        assert_eq!(
            app.get_link_far(r1, r2).unwrap(),
            "10.0.0.2/30".parse().unwrap(),
        );

        app.link(r2, r1, IpNet::from_str("10.0.0.4/30").unwrap());
        assert_eq!(
            app.get_link_close(r1, r2).unwrap(),
            "10.0.0.5/30".parse().unwrap(),
        );
        assert_eq!(
            app.get_link_far(r1, r2).unwrap(),
            "10.0.0.6/30".parse().unwrap(),
        );

        app.rip_enabled.push(r1);
        app.rip_enabled.push(r2);

        for (router_name, commands) in app.to_commands() {
            println!("{}:\n{}", router_name, commands);
        }

        todo!();
    }
}
