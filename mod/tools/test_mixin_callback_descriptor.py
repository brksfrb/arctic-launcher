"""Regression coverage for the callback descriptor check used by mod/check.py."""
import importlib.util
from pathlib import Path
import unittest


MOD = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("mixcheck", MOD / "versions/fabric/mixcheck.py")
mixcheck = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mixcheck)


class CallbackDescriptorTest(unittest.TestCase):
    def test_held_item_115_descriptor(self):
        cls = "net.minecraft.client.renderer.ItemInHandRenderer"
        jar = mixcheck.Jar("unused")
        jar.members_cache[cls] = """  private void renderArmWithItem(Player, float, PoseStack, MultiBufferSource, int);
    descriptor: (LPlayer;FLPoseStack;LMultiBufferSource;I)V
"""
        imports = {"CallbackInfo": mixcheck.CALLBACK, "CallbackInfoReturnable": mixcheck.CALLBACK + "Returnable"}
        invalid = (
            "Player player, float pitch, CallbackInfo ci",
            "Player player, float pitch, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfoReturnable<?> ci",
        )
        valid = (
            "Player player, float pitch, PoseStack pose, MultiBufferSource buffers, int light, CallbackInfo ci",
            "CallbackInfo ci",
            "CallbackInfo ci, @com.llamalad7.mixinextras.sugar.Local(argsOnly = true) PoseStack pose",
        )
        for params in invalid + valid:
            with self.subTest(params=params):
                source = f"""@Inject(method = "renderArmWithItem", at = @At("HEAD"), require = 0)
private void arctic$start({params}) {{}}"""
                problems = mixcheck.callback_problems(jar, cls, source, imports, {})
                self.assertEqual(params in invalid, bool(problems))


if __name__ == "__main__":
    unittest.main()
