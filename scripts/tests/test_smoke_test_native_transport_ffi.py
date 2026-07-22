import importlib.util
import struct
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "smoke_test_native_transport_ffi.py"


def load_smoke_script():
    spec = importlib.util.spec_from_file_location("smoke_test_native_transport_ffi", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class NativeRolePayloadTests(unittest.TestCase):
    def test_token_submit_payload_matches_frozen_layout(self):
        smoke = load_smoke_script()
        operation_id = 0x1020_3040_5060_7080
        payload = smoke.token_submit_payload(operation_id, b"submit")
        metadata = payload[: smoke.FRAME_SUBMIT_METADATA_LEN]

        self.assertEqual(len(metadata), 72)
        self.assertEqual(struct.unpack_from("<H", metadata, 16)[0], 25)
        self.assertEqual(struct.unpack_from("<Q", metadata, 40)[0], operation_id)
        self.assertEqual(metadata[52], 0)
        self.assertEqual(struct.unpack_from("<I", metadata, 64)[0], 2)
        self.assertEqual(struct.unpack_from("<H", metadata, 68)[0], 1)
        self.assertEqual(payload[72:], b"submit")

    def test_token_result_payload_matches_frozen_layout(self):
        smoke = load_smoke_script()
        payload = smoke.token_result_payload(b"result")
        metadata = payload[: smoke.RESULT_PUSH_METADATA_LEN]

        self.assertEqual(len(metadata), 64)
        self.assertEqual(struct.unpack_from("<H", metadata, 0)[0], 200)
        self.assertEqual(struct.unpack_from("<H", metadata, 8)[0], smoke.PROFILE_TOKEN)
        self.assertEqual(struct.unpack_from("<H", metadata, 12)[0], 3)
        self.assertEqual(struct.unpack_from("<H", metadata, 14)[0], 1)
        self.assertEqual(struct.unpack_from("<H", metadata, 16)[0], 4)
        self.assertEqual(metadata[44], 0)
        self.assertEqual(struct.unpack_from("<I", metadata, 56)[0], 2)
        self.assertEqual(struct.unpack_from("<H", metadata, 60)[0], 1)
        self.assertEqual(payload[64:], b"result")

    def test_secure_websocket_smoke_uses_certificate_server_name(self):
        smoke = load_smoke_script()

        self.assertEqual(smoke.SECURE_WEBSOCKET_ENDPOINT, b"wss://localhost:0/nnrp")


if __name__ == "__main__":
    unittest.main()
