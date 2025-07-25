// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use futures_util::stream::TryStreamExt;
use rtnetlink::packet_route::link::{
    AfSpecInet6, AfSpecUnspec, LinkAttribute, LinkMessage,
};
use serde::Serialize;

use super::flags::link_flags_to_string;
use iproute_rs::{
    CanDisplay, CanOutput, CliColor, CliError, mac_to_string, write_with_color,
};

#[derive(Serialize, Default)]
pub(crate) struct CliLinkInfoDetails {
    promiscuity: u32,
    min_mtu: u32,
    max_mtu: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    inet6_addr_gen_mode: String,
    num_tx_queues: u32,
    num_rx_queues: u32,
    gso_max_size: u32,
    gso_max_segs: u32,
}

#[derive(Serialize, Default)]
pub(crate) struct CliLinkInfo {
    ifindex: u32,
    ifname: String,
    flags: Vec<String>,
    mtu: u32,
    qdisc: String,
    #[serde(skip_serializing_if = "Option::is_none", rename = "master")]
    controller: Option<String>,
    #[serde(skip)]
    controller_ifindex: Option<u32>,
    operstate: String,
    linkmode: String,
    group: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    txqlen: Option<u32>,
    link_type: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    address: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    broadcast: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(flatten)]
    details: Option<CliLinkInfoDetails>,
}

impl std::fmt::Display for CliLinkInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: ", self.ifindex)?;
        write_with_color!(f, CliColor::IfaceName, "{}: ", self.ifname)?;
        write!(
            f,
            "<{}> mtu {} qdisc {}",
            self.flags.as_slice().join(","),
            self.mtu,
            self.qdisc,
        )?;
        if let Some(ctrl) = self.controller.as_ref() {
            write!(f, " master {ctrl}")?;
        }
        write!(f, " state ")?;
        if self.operstate == "UP" {
            write_with_color!(f, CliColor::StateUp, "{} ", self.operstate)?;
        } else if self.operstate == "DOWN" {
            write_with_color!(f, CliColor::StateDown, "{} ", self.operstate)?;
        } else {
            write!(f, "{} ", self.operstate)?;
        }
        write!(f, "mode {} group {} ", self.linkmode, self.group,)?;
        if let Some(v) = self.txqlen {
            write!(f, "qlen {v}")?;
        }
        write!(f, "\n    ")?;
        write!(f, "link/{} ", self.link_type)?;
        if !self.address.is_empty() {
            write_with_color!(f, CliColor::Mac, "{}", self.address)?;
            write!(f, " brd ")?;
            write_with_color!(f, CliColor::Mac, "{}", self.broadcast)?;
        }

        if let Some(details) = &self.details {
            write!(
                f,
                " promiscuity {} minmtu {} maxmtu {} addrgenmode {} numtxqueues {} numrxqueues {} gso_max_size {} gso_max_segs {} ",
                details.promiscuity,
                details.min_mtu,
                details.max_mtu,
                details.inet6_addr_gen_mode,
                details.num_tx_queues,
                details.num_rx_queues,
                details.gso_max_size,
                details.gso_max_segs
            )?;
        }
        Ok(())
    }
}

impl CanDisplay for CliLinkInfo {
    fn gen_string(&self) -> String {
        self.to_string()
    }
}

impl CanOutput for CliLinkInfo {}

pub(crate) async fn handle_show(
    _opts: &[&str],
    include_details: bool,
) -> Result<Vec<CliLinkInfo>, CliError> {
    let (connection, handle, _) = rtnetlink::new_connection()?;

    tokio::spawn(connection);

    let link_get_handle = handle.link().get();

    /*
    if let Some(iface_name) = filter.iface_name.as_ref() {
        link_get_handle = link_get_handle.match_name(iface_name.to_string());
    }
    */

    let mut links = link_get_handle.execute();
    let mut ifaces: Vec<CliLinkInfo> = Vec::new();

    while let Some(nl_msg) = links.try_next().await? {
        ifaces.push(parse_nl_msg_to_iface(nl_msg, include_details)?);
    }

    resolve_controller_name(&mut ifaces);

    Ok(ifaces)
}

pub(crate) fn parse_nl_msg_to_iface(
    nl_msg: LinkMessage,
    include_details: bool,
) -> Result<CliLinkInfo, CliError> {
    let mut ret = CliLinkInfo {
        ifindex: nl_msg.header.index,
        flags: link_flags_to_string(nl_msg.header.flags),
        link_type: nl_msg.header.link_layer_type.to_string().to_lowercase(),
        ..Default::default()
    };

    let mut details = CliLinkInfoDetails::default();

    for nl_attr in nl_msg.attributes {
        match nl_attr {
            LinkAttribute::IfName(name) => ret.ifname = name,
            LinkAttribute::Mtu(mtu) => ret.mtu = mtu,
            LinkAttribute::Address(mac) => ret.address = mac_to_string(&mac),
            LinkAttribute::Broadcast(mac) => {
                ret.broadcast = mac_to_string(&mac)
            }
            LinkAttribute::Qdisc(qdisc) => ret.qdisc = qdisc,
            LinkAttribute::OperState(state) => {
                // TODO: impl Display for State in rust-netlink
                ret.operstate = format!("{state:?}").to_uppercase()
            }
            LinkAttribute::TxQueueLen(v) => {
                if v > 0 {
                    ret.txqlen = Some(v)
                }
            }
            LinkAttribute::Group(v) => {
                ret.group = resolve_ip_link_group_name(v)
            }
            LinkAttribute::Mode(v) => ret.linkmode = v.to_string(),
            LinkAttribute::Controller(d) => ret.controller_ifindex = Some(d),
            LinkAttribute::Promiscuity(p) => details.promiscuity = p,
            LinkAttribute::MinMtu(m) => details.min_mtu = m,
            LinkAttribute::MaxMtu(m) => details.max_mtu = m,
            LinkAttribute::NumTxQueues(n) => details.num_tx_queues = n,
            LinkAttribute::NumRxQueues(n) => details.num_rx_queues = n,
            LinkAttribute::GsoMaxSize(g) => details.gso_max_size = g,
            LinkAttribute::GsoMaxSegs(g) => details.gso_max_segs = g,
            LinkAttribute::AfSpecUnspec(a) => {
                details.inet6_addr_gen_mode = get_addr_gen_mode(&a)
            }
            _ => {
                // println!("Remains {:?}", nl_attr);
            }
        }
    }

    ret.details = include_details.then_some(details);

    Ok(ret)
}

fn get_addr_gen_mode(af_spec_unspec: &[AfSpecUnspec]) -> String {
    af_spec_unspec
        .iter()
        .filter_map(|s| {
            let AfSpecUnspec::Inet6(v) = s else {
                return None;
            };
            v.iter()
                .filter_map(|i| {
                    if let AfSpecInet6::AddrGenMode(mode) = i {
                        Some(mode)
                    } else {
                        None
                    }
                })
                .next()
        })
        .next()
        .copied()
        .unwrap_or_default()
        .to_string()
}

fn resolve_ip_link_group_name(id: u32) -> String {
    // TODO: Read `/usr/share/iproute2/group` and `/etc/iproute2/group`
    match id {
        0 => "default".into(),
        _ => id.to_string(),
    }
}

fn resolve_controller_name(links: &mut [CliLinkInfo]) {
    let index_2_name: HashMap<u32, String> = links
        .iter()
        .map(|l| (l.ifindex, l.ifname.to_string()))
        .collect();

    for link in links.iter_mut() {
        if let Some(ctrl_ifindex) = link.controller_ifindex
            && let Some(name) = index_2_name.get(&ctrl_ifindex)
        {
            link.controller = Some(name.to_string());
        }
    }
}
