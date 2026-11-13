"""Regression checks for the private-desktop automation's report channel."""
import ctypes
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import threading
import time
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location("auto_world", Path(__file__).with_name("auto-world.py"))
auto_world = importlib.util.module_from_spec(spec)
spec.loader.exec_module(auto_world)


class ReportChannel(unittest.TestCase):
    def test_fog_evaluation_rejects_stale_time_or_a_different_held_baseline(self):
        timeline={"nativeOrigin":[262.25,.03,.0009,267.75],"replayOrigin":22.,"scale":1.,
                  "scaledOrigin":262.25,"scaledOriginReplay":22.,"lastReplay":22.5,"value":[262.75,0.,0.,268.25]}
        fog={"session":"native_1_1","generation":7,"seconds":22.5,"verified":True,
             "nativeFunctionEvaluated":True,"shaderConstantsOverridden":False,"repeated":True,
             "startingStateRestored":True,"baselineSha256":"a"*64}
        timing={"rendererClock":timeline["value"],"elapsedMs":22500,"partial":0.,"ticksPerSecond":20,
                "rendererState":{"clockMapping":{"session":"native_1_1","generation":7,"verified":True,"timeline":timeline},
                                 "fogEvaluation":fog}}
        self.assertIs(auto_world.matching_fog_evaluation(timing,"native_1_1",7,"a"*64),fog)
        self.assertIsNone(auto_world.matching_fog_evaluation(timing,"native_1_2",7))
        self.assertIsNone(auto_world.matching_fog_evaluation(timing,"native_1_1",8))
        self.assertIsNone(auto_world.matching_fog_evaluation(timing,"native_1_1",7,"b"*64))
        fog["seconds"]=22.4
        self.assertIsNone(auto_world.matching_fog_evaluation(timing,"native_1_1",7))
        fog["seconds"]=22.5;fog["repeated"]=False
        self.assertIsNone(auto_world.matching_fog_evaluation(timing,"native_1_1",7))

    def test_renderer_clock_uses_its_native_epochs_and_rejects_a_client_time_reset(self):
        timeline={"nativeOrigin":[262.25,.03,.0009,267.75],"replayOrigin":22.,"scale":1.,
                  "scaledOrigin":262.25,"scaledOriginReplay":22.,"lastReplay":22.5,"value":[262.75,0.,0.,268.]}
        mapping={"session":"native_1_1","generation":7,"verified":True,"timeline":timeline}
        timing={"rendererClock":[262.75,0.,0.,268.25],"rendererState":{"clockMapping":mapping},
                "elapsedMs":22500,"partial":0.,"ticksPerSecond":20}
        timeline["value"]=timing["rendererClock"].copy()
        self.assertIs(auto_world.matching_renderer_mapping(timing,"native_1_1",7),mapping)
        self.assertIsNone(auto_world.matching_renderer_mapping(timing,"native_1_2",7))
        self.assertIsNone(auto_world.matching_renderer_mapping(timing,"native_1_1",8))
        timing["rendererClock"]=[22.5,0.,0.,22.5]
        self.assertIsNone(auto_world.matching_renderer_mapping(timing,"native_1_1",7))

    def test_world_report_at_last_poll_is_read_before_reporting_timeout(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe)
            probe.root=Path(directory);probe.live={"pid":123};probe.deadline=100
            probe.check=Mock()
            report={"schema":1,"sequence":17,"world":{"session":"native_123_1"}}
            def complete(_):
                (probe.root/"probe-world-17.json").write_text(json.dumps(report))
            with patch.object(auto_world.time,"monotonic",side_effect=[0,0,4.99,5.09]), \
                 patch.object(auto_world.time,"sleep",side_effect=complete) as sleep, \
                 patch.object(auto_world,"write",wraps=auto_world.write) as write:
                self.assertEqual(probe.world_snapshot(17),report)
            self.assertEqual(write.call_count,1)
            self.assertLessEqual(sleep.call_args.args[0],.011)

    def test_slow_startup_snapshot_waits_without_sending_another_key(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe);probe.profile=Path(directory)
            probe.deadline=100;probe.check=Mock();probe.input=Mock()
            def present(_):
                (probe.profile/"frame.rgba").write_bytes(bytes([0,0,0,255])*4)
                (probe.profile/"probe-frame-1.json").write_text(json.dumps({"format":"RGBA8","file":"frame.rgba","width":2,"height":2}))
            with patch.object(auto_world.time,"monotonic",side_effect=[0,11,12]),patch.object(auto_world.time,"sleep",side_effect=present):
                image=probe.snapshot(ui_states=("main_menu",))
            self.assertEqual(image.size,(2,2));probe.input.assert_not_called()

    def test_menu_transition_keeps_pending_capture_until_stage_deadline(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe);probe.profile=Path(directory)
            probe.deadline=100;probe.check=Mock();probe.input=Mock()
            def present(_):
                self.assertTrue((probe.profile/"probe-snapshot.request").exists())
                (probe.profile/"frame.rgba").write_bytes(bytes([0,0,0,255])*4)
                (probe.profile/"probe-frame-1.json").write_text(json.dumps({"format":"RGBA8","file":"frame.rgba","width":2,"height":2}))
            with patch.object(auto_world.time,"monotonic",side_effect=[31,61]),patch.object(auto_world.time,"sleep",side_effect=present):
                image=probe.snapshot(ui_states=("main_menu",),until=90)
            self.assertEqual(image.size,(2,2));probe.input.assert_not_called()

    def test_menu_wait_remains_bounded_and_does_not_extend_marked_world_wait(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe);probe.profile=Path(directory)
            probe.deadline=100;probe.check=Mock()
            with patch.object(auto_world.time,"monotonic",return_value=91):
                with self.assertRaisesRegex(TimeoutError,"owned GPU frame did not arrive"):
                    probe.snapshot(ui_states=("main_menu",),until=90)
            with patch.object(auto_world.time,"monotonic",side_effect=[0,11]):
                with self.assertRaisesRegex(TimeoutError,"owned GPU frame did not arrive"):
                    probe.snapshot(world_request=7,until=90)

    def test_failed_indexed_export_stops_encoder_releases_camera_and_restores_hud(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe);probe.root=Path(directory);probe.publish=Mock()
            probe.ffmpeg=probe.root/"ffmpeg.exe";probe.input=Mock();probe.capture_command=Mock(return_value={})
            world={"session":"current","camera":[0,1,2,0,0,70],"clockMs":50,
                   "nativeTiming":{"ticks":1,"partial":.25,"cameraComponent":{"camera":{"orientation":[0,1,0,0]}}}}
            probe.world_snapshot=Mock(return_value={"world":world})
            probe.clock_control=Mock(return_value={})
            probe.camera_command=Mock(side_effect=[RuntimeError("native camera expired"),{}])
            with self.assertRaisesRegex(RuntimeError,"native camera expired"):
                probe.verify_indexed_export()
            self.assertEqual([c.args[0]["op"] for c in probe.capture_command.call_args_list],["start","stop"])
            self.assertEqual([c.args[0] for c in probe.camera_command.call_args_list],["set","release"])
            self.assertEqual([c.args[0] for c in probe.clock_control.call_args_list],["freeze","resume"])
            self.assertEqual([c.args for c in probe.input.call_args_list],[("key",112),("key",112)])
            evidence=auto_world.load(probe.root/"indexed-export-evidence.json")
            self.assertFalse(evidence["diagnosticIndexedGpuExportObserved"])
            self.assertFalse(evidence["deterministicVideoVerified"])

    def test_capture_ignores_a_receipt_for_another_output(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe);probe.root=Path(directory);probe.live={"pid":1}
            probe.check=Mock();probe.deadline=time.monotonic()+5;output=probe.root/"current.mp4"
            request=probe.root/"arctic-bedrock-1.capture-request.json"
            status=probe.root/"arctic-bedrock-1.capture.json"
            auto_world.write(status,{"output":"expired.mp4","frames":1,"receipt":{"index":0}})
            def acknowledge():
                until=time.monotonic()+2
                while time.monotonic()<until:
                    if request.exists():
                        request.unlink();time.sleep(.1)
                        auto_world.write(status,{"output":str(output),"frames":1,"receipt":{"index":0}});return
                    time.sleep(.005)
            worker=threading.Thread(target=acknowledge);worker.start()
            try:ack=probe.capture_command({"op":"sample","index":0},output,lambda s:s.get("frames")==1)
            finally:worker.join()
            self.assertEqual(ack["output"],str(output))

    def test_camera_release_uses_a_new_sequence_after_a_missing_acknowledgement(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe);probe.root=Path(directory);probe.body=True
            probe.live={"pid":1};probe.deadline=time.monotonic()+5;probe.actions=[];probe.publish=Mock()
            auto_world.write(probe.root/"arctic-offline-probe.json",{"body_probe":True})
            with patch.object(auto_world.time,"monotonic",side_effect=[0,0,6]):
                with self.assertRaises(TimeoutError):
                    probe.camera_command("set","current",{"position":[0,1,2],"orientation":[0,1,0,0],"fieldOfView":50})
            def acknowledge():
                until=time.monotonic()+2
                while time.monotonic()<until:
                    request=auto_world.load(probe.root/"probe-camera-control.json") or {}
                    if request.get("sequence")==2:
                        auto_world.write(probe.root/"probe-camera-control-result.json",{**request,"active":False,"restored":True});return
                    time.sleep(.005)
            worker=threading.Thread(target=acknowledge);worker.start()
            try:ack=probe.camera_command("release","current")
            finally:worker.join()
            self.assertEqual(ack["sequence"],2)
            self.assertEqual(probe.actions[0]["op"],"release")

    def test_failed_native_camera_control_releases_camera_and_resumes_clock(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe);probe.root=Path(directory);probe.publish=Mock()
            world={"session":"current","camera":[0,1,2,0,0,70],"nativeTiming":{"partial":.25,"cameraComponent":{"camera":{"orientation":[0,1,0,0]}}}}
            probe.world_snapshot=Mock(return_value={"world":world,"sequence":2710})
            probe.snapshot=Mock(return_value=auto_world.Image.new("RGB",(20,20)))
            probe.clock_control=Mock(return_value={})
            probe.camera_command=Mock(side_effect=[RuntimeError("wrong player pose"),{}])
            with self.assertRaisesRegex(RuntimeError,"wrong player pose"):
                probe.verify_camera_control()
            self.assertEqual([call.args[0] for call in probe.camera_command.call_args_list],["set","release"])
            self.assertEqual([call.args[0] for call in probe.clock_control.call_args_list],["freeze","resume"])
            evidence=auto_world.load(probe.root/"camera-control-evidence.json")
            self.assertFalse(evidence["diagnosticNativeCameraControlObserved"])
            self.assertEqual(evidence["error"],"wrong player pose")

    def test_foreign_camera_session_fails_and_restores_the_view(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe)
            probe.root=Path(directory);probe.publish=Mock();probe.snapshot=Mock(return_value=Mock(width=200,height=100));probe.input=Mock()
            camera={"orientation":[0,0,0,1],"position":[0,1,2],"aspectRatio":2,"fieldOfView":70}
            world={"session":"current","localPlayer":"player","width":200,"height":100,
                   "camera":[0,1,2,0,0,70],"nativeTiming":{"cameraComponent":{
                       "session":"current","player":"player","ownedTarget":True,
                       "weakEntityComponentMatch":True,"camera":camera}}}
            foreign=json.loads(json.dumps(world));foreign["nativeTiming"]["cameraComponent"]["session"]="expired"
            probe.world_snapshot=Mock(side_effect=[{"world":world},{"world":foreign}])
            with patch.object(auto_world.time,"sleep"):
                with self.assertRaisesRegex(RuntimeError,"crossed player/session ownership"):
                    probe.verify_camera_observer()
            self.assertEqual(probe.input.call_count,3)
            evidence=auto_world.load(probe.root/"camera-evidence.json")
            self.assertFalse(evidence["diagnosticNativeCameraObserved"])
            self.assertEqual(len(evidence["samples"]),1)
            self.assertFalse(evidence["nativeCameraControlVerified"])

    def test_native_clock_ack_from_another_session_cannot_authorize_a_step(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe)
            probe.root=Path(directory);probe.body=True;probe.live={"pid":1}
            probe.deadline=time.monotonic()+5;probe.actions=[];probe.publish=Mock()
            auto_world.write(probe.root/"arctic-offline-probe.json",{"body_probe":True})
            def acknowledge():
                until=time.monotonic()+2
                while time.monotonic()<until:
                    request=auto_world.load(probe.root/"probe-clock-control.json")
                    if request:
                        auto_world.write(probe.root/"probe-clock-control-result.json",{**request,"session":"old","frozen":True,"timer":{"alpha":.25,"ticks":1}});return
                    time.sleep(.005)
            worker=threading.Thread(target=acknowledge);worker.start()
            try:
                with self.assertRaisesRegex(RuntimeError,"acknowledgement differs"):
                    probe.clock_control("step","current",.25)
            finally:worker.join()
            self.assertEqual(probe.actions,[])
            with self.assertRaisesRegex(ValueError,"invalid clock control"):
                probe.clock_control("freeze","current",float("nan"))

    def test_failed_comparison_preserves_failure_and_runs_remaining_diagnostics(self):
        probe=auto_world.Probe.__new__(auto_world.Probe);probe.publish=Mock()
        failed=Mock(side_effect=RuntimeError("depth unchanged"));later=Mock();disabled=Mock()
        probe.collect_checks([("absence",True,failed),("emote",True,later),("disabled",False,disabled)])
        later.assert_called_once();disabled.assert_not_called()
        self.assertEqual(probe.failed_checks,[{"check":"absence","error":"depth unchanged"}])
        probe.publish.assert_called_once()

    def test_body_control_requires_explicit_probe_and_matching_jvm_ack(self):
        with tempfile.TemporaryDirectory() as directory:
            probe=auto_world.Probe.__new__(auto_world.Probe)
            probe.root=Path(directory);probe.body=True;probe.live={"pid":1}
            probe.deadline=time.monotonic()+5;probe.actions=[];probe.publish=Mock()
            auto_world.write(probe.root/"arctic-offline-probe.json",{"body_probe":True})
            auto_world.write(probe.root/"probe-body-control-result.json",{"schema":1,"sequence":0,"mode":"native","alpha":.0001})
            def acknowledge():
                until=time.monotonic()+2
                while time.monotonic()<until:
                    request=auto_world.load(probe.root/"probe-body-control.json")
                    if request:
                        auto_world.write(probe.root/"probe-body-control-result.json",request);return
                    time.sleep(.005)
            worker=threading.Thread(target=acknowledge);worker.start()
            try:probe.body_control("suppression",.001)
            finally:worker.join()
            self.assertEqual(probe.actions[0]["mode"],"suppression")
            self.assertEqual(probe.actions[0]["sequence"],1)
            with self.assertRaisesRegex(ValueError,"invalid body control"):
                probe.body_control("replacement",float("nan"))
            probe.body=False
            with self.assertRaisesRegex(ValueError,"explicit isolated body probe"):
                probe.body_control("native")

    def test_navigation_uses_one_capture_for_classification_and_evidence(self):
        probe = auto_world.Probe.__new__(auto_world.Probe)
        probe.deadline = time.monotonic() + 10
        probe.publish = Mock()
        probe.snapshot = Mock(return_value=object())
        with patch.object(auto_world, "classify", return_value="main_menu"):
            self.assertEqual(probe.wait_ui("main_menu"), "main_menu")
        self.assertEqual(probe.snapshot.call_count,1)
        self.assertEqual(probe.snapshot.call_args.kwargs["ui_states"],("main_menu",))
        self.assertEqual(probe.snapshot.call_args.kwargs["until"],probe.deadline)

    def test_native_bootstrap_failure_is_seen_before_live_pid_or_engine_report(self):
        with tempfile.TemporaryDirectory() as directory:
            probe = auto_world.Probe.__new__(auto_world.Probe)
            probe.root = Path(directory)
            probe.live = None
            probe.deadline = time.monotonic() + 60
            (probe.root / "arctic-bedrock-123.log").write_text("starting native adapter\nstartup failed: rejected probe field\n")
            with self.assertRaisesRegex(RuntimeError, "startup failed: rejected probe field"):
                probe.check()

    def test_snapshot_requires_the_marker_of_the_exact_world_layer(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            probe = auto_world.Probe.__new__(auto_world.Probe)
            probe.root = probe.profile = root
            probe.live = None
            probe.deadline = time.monotonic() + 10
            for number, marker, color in [(1, 71, (10, 20, 30, 255)), (2, 72, (200, 0, 0, 255))]:
                name = f"frame-{number}.rgba"
                (root / name).write_bytes(bytes(color))
                (root / f"probe-frame-{number}.json").write_text(json.dumps({
                    "format": "RGBA8", "file": name, "width": 1, "height": 1,
                    "worldRequestSequence": marker}))
            image = probe.snapshot(world_request=71)
            self.assertEqual(image.getpixel((0, 0)), (10, 20, 30, 255))
            self.assertFalse((root / "probe-snapshot.request").exists())
            probe.frames = []
            probe.publish = Mock()
            with patch.object(auto_world, "classify", return_value="main_menu"):
                probe.snapshot(world_request=71, ui_states=("main_menu",))
            saved = json.loads((root / "auto-world-main_menu.json").read_text())
            self.assertEqual(saved["worldRequestSequence"], 71)
            self.assertEqual(saved["sourceMetadata"], "probe-frame-1.json")
            self.assertEqual(probe.frames, ["auto-world-main_menu.png"])

    def test_later_feature_failure_preserves_completed_live_checks(self):
        with tempfile.TemporaryDirectory() as directory:
            probe = auto_world.Probe.__new__(auto_world.Probe)
            probe.report = Path(directory) / "report.json"
            probe.stage = "error"
            probe.actions = []
            probe.frames = []
            probe.world_hooks_observed = True
            probe.world_depth_observed = True
            probe.cosmetic_observed = True
            probe.cape_observed = False
            probe.world_prepared = False
            probe.global_observed = False
            probe.movement_observed = False
            probe.moving_cape_observed = False
            probe.movement_clock_observed = False
            probe.body_observed = False
            probe.publish(message="cape verification failed")
            report = json.loads(probe.report.read_text())
            self.assertEqual(report["state"], "error")
            self.assertTrue(report["worldHooksObserved"])
            self.assertTrue(report["worldDepthObserved"])
            self.assertTrue(report["diagnosticCosmeticObserved"])
            self.assertFalse(report["animatedCapeObserved"])
            self.assertFalse(report["cosmeticsVerified"])
            self.assertFalse(report["deterministicVideoVerified"])
            self.assertTrue(report["navigationAutomated"])
            with patch.object(auto_world.time, "monotonic", return_value=probe.started_at + 2):
                probe.stage = "complete"
                probe.publish()
            report = json.loads(probe.report.read_text())
            self.assertEqual(report["stageDurationsMs"]["error"], 2000)
            self.assertEqual(report["elapsedMs"], 2000)

    @unittest.skipUnless(os.name == "nt", "Windows file sharing regression")
    def test_report_survives_reader_without_delete_sharing(self):
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        create = kernel.CreateFileW
        create.argtypes = [ctypes.c_wchar_p, ctypes.c_uint32, ctypes.c_uint32,
                           ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p]
        create.restype = ctypes.c_void_p
        close = kernel.CloseHandle
        close.argtypes = [ctypes.c_void_p]
        close.restype = ctypes.c_int
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.json"
            auto_world.write(path, {"state": "old"})
            handle = create(str(path), 0x80000000, 3, None, 3, 0, None)
            self.assertNotEqual(handle, ctypes.c_void_p(-1).value)
            # The inspector must always see a whole report while replacement
            # waits for this real Windows handle to release the destination.
            timer = threading.Timer(.15, lambda: close(handle))
            timer.start()
            try:
                auto_world.write(path, {"state": "complete", "worldPasses": 30})
            finally:
                timer.join()
            self.assertEqual(json.loads(path.read_text()), {"state": "complete", "worldPasses": 30})
            self.assertFalse(path.with_suffix(".json.tmp").exists())


if __name__ == "__main__":
    unittest.main()
