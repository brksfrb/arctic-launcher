import os
import socket
import unittest
from lan_probe import owned_udp_endpoints


@unittest.skipUnless(os.name=="nt","Windows UDP ownership API")
class KernelOwnership(unittest.TestCase):
    def test_real_bound_endpoint_is_joined_to_its_process_id(self):
        with socket.socket(socket.AF_INET,socket.SOCK_DGRAM) as udp:
            udp.bind(("127.0.0.1",0));port=udp.getsockname()[1]
            self.assertIn({"pid":os.getpid(),"address":"127.0.0.1","port":port},owned_udp_endpoints(os.getpid()))
        self.assertNotIn({"pid":os.getpid(),"address":"127.0.0.1","port":port},owned_udp_endpoints(os.getpid()))
        with self.assertRaises(ValueError):owned_udp_endpoints(0)

    def test_real_ipv6_endpoint_is_not_lost_by_the_ipv4_table(self):
        with socket.socket(socket.AF_INET6,socket.SOCK_DGRAM) as udp:
            udp.bind(("::1",0));port=udp.getsockname()[1]
            endpoint={"pid":os.getpid(),"address":"::1","port":port}
            self.assertIn(endpoint,owned_udp_endpoints(os.getpid()))
        self.assertNotIn(endpoint,owned_udp_endpoints(os.getpid()))

    def test_dual_stack_mapped_loopback_keeps_its_kernel_witness(self):
        with socket.socket(socket.AF_INET6,socket.SOCK_DGRAM) as udp:
            udp.setsockopt(socket.IPPROTO_IPV6,socket.IPV6_V6ONLY,0)
            udp.bind(("::ffff:127.0.0.1",0));port=udp.getsockname()[1]
            # Windows reports this mapped endpoint in its IPv4 owner table.
            self.assertIn({"pid":os.getpid(),"address":"127.0.0.1","port":port},owned_udp_endpoints(os.getpid()))
