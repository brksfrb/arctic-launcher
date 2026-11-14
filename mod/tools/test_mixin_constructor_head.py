"""Regression coverage for the per-target mixin guard (no game download needed)."""
import importlib.util
from pathlib import Path
import unittest


MOD = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("mixcheck", MOD / "versions/fabric/mixcheck.py")
mixcheck = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mixcheck)


class ConstructorHeadTest(unittest.TestCase):
    def test_glass_regression(self):
        source = (MOD / "versions/fabric/src/main/java/com/arcticlauncher/mod/mixin/GlassOldImageMixin.java").read_text()
        self.assertEqual([], mixcheck.constructor_head_problems(source))
        broken = source.replace("private static NativeImage arctic$image", "private NativeImage arctic$image")
        self.assertEqual(1, len(mixcheck.constructor_head_problems(broken)))
        self.assertIn("arctic$image", mixcheck.constructor_head_problems(broken)[0])

    def test_injection_forms(self):
        for injector in ("ModifyVariable", "Inject", "Redirect", "ModifyArg", "ModifyArgs",
                         "ModifyConstant", "WrapOperation", "WrapWithCondition", "ModifyExpressionValue"):
            for selector in ('"<init>"', '"<init>(I)V"', '{"other", "<init>(I)V"}', 'CTOR'):
                with self.subTest(injector=injector, selector=selector):
                    source = f'''private static final String CTOR = "<init>(I)V";
@{injector}(method = {selector}, at = @At(value = "HEAD"), argsOnly = true)
private int hook(int value, @Local(argsOnly = true) Info info) {{ return value; }}'''
                    self.assertEqual(1, len(mixcheck.constructor_head_problems(source)))
                    self.assertEqual([], mixcheck.constructor_head_problems(source.replace("private int hook", "private static int hook")))

    def test_instance_handlers_after_super_and_other_methods(self):
        for method, at in (("<init>", "RETURN"), ("<init>", "TAIL"), ("loadData", "HEAD")):
            with self.subTest(method=method, at=at):
                source = f'''@Inject(method = "{method}", at = @At("{at}"))
private void hook(CallbackInfo ci) {{}}'''
                self.assertEqual([], mixcheck.constructor_head_problems(source))


if __name__ == "__main__":
    unittest.main()
