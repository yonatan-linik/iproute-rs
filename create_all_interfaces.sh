#!/bin/bash

set -e

NETNS=testns
VETH_HOST=veth-host
VETH_NS=veth-ns

echo "[+] Creating network namespace: $NETNS"
ip netns add $NETNS

echo "[+] Creating veth pair: $VETH_HOST <-> $VETH_NS"
ip link add $VETH_HOST type veth peer name $VETH_NS

echo "[+] Moving $VETH_NS to $NETNS"
ip link set $VETH_NS netns $NETNS

echo "[+] Bringing up $VETH_HOST in root namespace"
ip addr add 10.200.1.1/24 dev $VETH_HOST
ip link set $VETH_HOST up

echo "[+] Configuring $VETH_NS inside $NETNS"
ip netns exec $NETNS ip link set $VETH_NS up
ip netns exec $NETNS ip link set lo up

# Keep the base interface up, do NOT bring it down
echo "[+] Creating VLAN on $VETH_NS (vlan id 100)"
ip netns exec $NETNS ip link add link $VETH_NS name ${VETH_NS}.100 type vlan id 100
ip netns exec $NETNS ip link set ${VETH_NS}.100 up

echo "[+] Creating MACVLAN on $VETH_NS"
ip netns exec $NETNS ip link add macv0 link $VETH_NS type macvlan mode bridge
ip netns exec $NETNS ip link set macv0 up

echo "[+] Creating IPVLan on $VETH_NS"
ip netns exec $NETNS ip link add ipv0 link $VETH_NS type ipvlan mode l2
ip netns exec $NETNS ip link set ipv0 up

# Now assign IPs to the base interface and sub-interfaces
echo "[+] Configuring IPs on $VETH_NS and its sub-interfaces"
ip netns exec $NETNS ip addr add 10.200.1.2/24 dev $VETH_NS
ip netns exec $NETNS ip addr add 10.200.1.3/24 dev ${VETH_NS}.100  # Example IP for VLAN

echo "[+] Creating Dummy interface"
ip netns exec $NETNS ip link add dummy0 type dummy
ip netns exec $NETNS ip link set dummy0 up

echo "[+] Creating Bridge interface"
ip netns exec $NETNS ip link add br0 type bridge
ip netns exec $NETNS ip link set br0 up

echo "[+] Creating VETH pair inside $NETNS"
ip netns exec $NETNS ip link add veth0 type veth peer name veth1
ip netns exec $NETNS ip link set veth0 up
ip netns exec $NETNS ip link set veth1 up

echo "[+] Creating TUN interface"
ip netns exec $NETNS ip tuntap add dev tun0 mode tun
ip netns exec $NETNS ip link set tun0 up

echo "[+] Creating TAP interface"
ip netns exec $NETNS ip tuntap add dev tap0 mode tap
ip netns exec $NETNS ip link set tap0 up

echo "[+] Creating GRE tunnel"
ip netns exec $NETNS ip tunnel add gre1 mode gre remote 192.0.2.2 local 192.0.2.1 ttl 255
ip netns exec $NETNS ip link set gre1 up

echo "[+] Creating IPIP tunnel"
ip netns exec $NETNS ip tunnel add ipip1 mode ipip remote 192.0.2.2 local 192.0.2.1
ip netns exec $NETNS ip link set ipip1 up

echo "[+] Creating SIT tunnel"
ip netns exec $NETNS ip tunnel add sit1 mode sit remote 192.0.2.2 local 192.0.2.1
ip netns exec $NETNS ip link set sit1 up

echo "[+] Creating GRETAP interface"
ip netns exec $NETNS ip link add gretap1 type gretap remote 192.0.2.2 local 192.0.2.1
ip netns exec $NETNS ip link set gretap1 up

echo "[+] Creating VRF interface"
ip netns exec $NETNS ip link add vrf-blue type vrf table 1001
ip netns exec $NETNS ip link set vrf-blue up

echo "[+] Done. Interfaces in $NETNS:"
ip netns exec $NETNS ip link
