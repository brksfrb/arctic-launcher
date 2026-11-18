"""Regression coverage for the callback descriptor check used by mod/check.py."""
import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
from zipfile import ZipFile


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

    def test_inherited_method_keeps_argument_count_check_with_coerce(self):
        cls = "net.minecraft.Child"
        jar = mixcheck.Jar("unused")
        jar.members_cache[cls] = f"public class {cls} extends net.minecraft.Parent {{\n}}"
        jar.members_cache["net.minecraft.Parent"] = """  protected void hook(java.lang.String, int);
    descriptor: (Ljava/lang/String;I)V
"""
        imports = {"CallbackInfo": mixcheck.CALLBACK}
        for params, problem_count in (
            ("@Coerce Object value, int number, CallbackInfo ci", 0),
            ("@Coerce Object value, CallbackInfo ci", 1),
            ("Object value, int number, CallbackInfo ci", 1),
        ):
            with self.subTest(params=params):
                source = f'''@Inject(method = "hook", at = @At("HEAD"))
private void handler({params}) {{}}'''
                self.assertEqual(len(mixcheck.callback_problems(jar, cls, source, imports, {})), problem_count)

    def test_processed_mixins_take_precedence_for_the_matching_target(self):
        config = "arctic.mixins.json"
        cases = (
            ("1.20.1", "arctic-mod-1.20.1.jar", "fabric.mod.json",
             json.dumps({"depends": {"minecraft": ["1.20", "1.20.1"]}})),
            ("neoforge-1.20.1", "arctic-neoforge-1.20.1.jar", "META-INF/mods.toml",
             'modId = "minecraft"\nmandatory = true\nversionRange = "[1.20,1.20.1]"\n'),
            ("neoforge-26.3", "arctic-neoforge-26.3.jar", "META-INF/neoforge.mods.toml",
             'modId = "minecraft"\ntype = "required"\nversionRange = "[26.3]"\n'),
        )
        for version, jar, metadata, content in cases:
            with self.subTest(version=version), TemporaryDirectory() as directory:
                mod = Path(directory)
                project = mod / "versions" / "fabric"
                resources = project / "build" / "resources" / "main"
                metadata_path = resources / metadata
                metadata_path.parent.mkdir(parents=True)
                (mod / "dist").mkdir()
                (resources / config).write_text('{"client": ["ProcessedMixin"]}', encoding="utf-8")
                metadata_path.write_text(content, encoding="utf-8")
                with ZipFile(mod / "dist" / jar, "w") as built:
                    built.writestr(config, '{"client": ["JarMixin"]}')
                with patch.object(mixcheck, "HERE", str(project)), patch.object(mixcheck, "build_dir", return_value=project):
                    self.assertEqual(mixcheck.active_mixins(version), {"ProcessedMixin"})
                    metadata_path.write_text(content.replace("1.20.1", "1.20.2").replace("26.3", "26.2"), encoding="utf-8")
                    self.assertEqual(mixcheck.active_mixins(version), {"JarMixin"})


if __name__ == "__main__":
    unittest.main()
