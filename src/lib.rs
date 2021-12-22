use std::net::IpAddr;

use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use slotmap::{DefaultKey, SlotMap};

#[derive(Default, Debug)]
pub struct Device {
    name: String,
    links: Vec<(IpNet, DefaultKey)>,
}

#[derive(Debug)]
pub struct Link(IpNet);

pub struct App {
    pub devices: SlotMap<DefaultKey, Device>,
    pub links: SlotMap<DefaultKey, Link>,
    pub rip_enabled: Vec<DefaultKey>,
}

impl App {
    /// Create a new `App`, without devices or links
    pub fn new() -> Self {
        App { devices: SlotMap::new(), links: SlotMap::new() }
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
    pub fn connect(&mut self, r1: DefaultKey, r2: DefaultKey, ip: IpNet) {
        let l1 = &self.devices[r1].links;
        let l2 = &self.devices[r2].links;

        // Ricerca link comune tra r1 ed r2
        let mut common = None;
        for i in 0..l1.len() {
            for j in 0..l2.len() {
                if l1[i].1 == l2[j].1 {
                    common = Some((i, j));
                }
            }
        }

        // Aggiornamento ip
        if let Some((i, j)) = common {
            let link = self.devices[r1].links.get(i).unwrap().1;
            self.links[link].0 = ip;

            let mut hosts = ip.hosts();
            self.devices[r1].links[i] = (to_ipnet(hosts.next().unwrap(), ip.prefix_len()), link);
            self.devices[r2].links[j] = (to_ipnet(hosts.next().unwrap(), ip.prefix_len()), link);
        } else {
            let link = self.links.insert(Link(ip));
            self.links[link].0 = ip;

            let mut hosts = ip.hosts();
            self.devices[r1]
                .links
                .push((to_ipnet(hosts.next().unwrap(), ip.prefix_len()), link));
            self.devices[r2]
                .links
                .push((to_ipnet(hosts.next().unwrap(), ip.prefix_len()), link));
        }
    }

    /// Disconnect the two devices if they are connected
    pub fn disconnect(&mut self, r1: DefaultKey, r2: DefaultKey) {
        let l1 = &self.devices[r1].links;
        let l2 = &self.devices[r2].links;
        let mut common = None;

        for i in 0..l1.len() {
            for j in 0..l2.len() {
                if l1[i].1 == l2[j].1 {
                    common = Some((i, j));
                }
            }
        }

        if let Some((i, j)) = common {
            let link = l1[i].1;

            self.devices[r1].links.remove(i);
            self.devices[r2].links.remove(j);
            self.links.remove(link);
        }
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
    use slotmap::SlotMap;

    #[test]
    fn add_remove_ip() {
        let devices = SlotMap::new();
        let links = SlotMap::new();
        let mut app = App { devices, links };

        let r1 = app.add_device(Device {
            name: "R1".to_string(),
            links: vec![],
        });
        let r2 = app.add_device(Device {
            name: "R2".to_string(),
            links: vec![],
        });

        app.connect(r1, r2, IpNet::from_str("10.0.0.0/30").unwrap());
        assert_eq!(
            app.get_device("R1").unwrap().links[0].0,
            "10.0.0.1/30".parse().unwrap(),
        );
        assert_eq!(app.links.len(), 1);

        app.connect(r1, r2, IpNet::from_str("10.0.0.4/30").unwrap());
        assert_eq!(
            app.get_device("R1").unwrap().links[0].0,
            "10.0.0.5/30".parse().unwrap(),
        );
        assert_eq!(app.links.len(), 1);

        app.disconnect(r1, r2);
        assert_eq!(app.get_device("R1").unwrap().links.len(), 0);
        assert_eq!(app.links.len(), 0);
    }
}