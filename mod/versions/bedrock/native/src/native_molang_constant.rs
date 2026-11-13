//! Read a pinned ConstantExpression's stored hash without calling Molang.
//! Dynamic/source/compiled expressions and float variants remain unsupported.
use crate::native_scene_layout::constant_expression as layout;
#[path = "native_molang_constant_profile.rs"]
mod profile;

/// `capture` must retain/recheck both the owning ExpressionNode bytes and this
/// object's bytes in the caller's synchronous native draw. No pointer is returned.
pub(crate) fn hash(
    expression: &[u8],
    mut capture: impl FnMut(usize, usize) -> Option<Vec<u8>>,
) -> Option<u64> {
    if expression.len() != layout::EXPRESSION_BYTES || expression[layout::EXPRESSION_VARIANT] != 0 {
        return None;
    }
    let expected = profile::table()?;
    let object = usize::try_from(u64::from_le_bytes(expression[..8].try_into().ok()?)).ok()?;
    let bytes = capture(object, layout::OBJECT_READ_BYTES)?;
    let table = usize::try_from(u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?)).ok()?;
    if table != expected
        || u32::from_le_bytes(
            bytes
                .get(layout::VALUE_TAG..layout::VALUE_TAG + 4)?
                .try_into()
                .ok()?,
        ) != layout::HASH_TAG
    {
        return None;
    }
    Some(u64::from_le_bytes(
        bytes
            .get(layout::VALUE_HASH..layout::VALUE_HASH + 8)?
            .try_into()
            .ok()?,
    ))
}
pub(crate) fn validate_profile(base: usize) -> Result<(), String> {
    profile::install(base)
}
pub(crate) fn reader_installed() -> bool {
    profile::table().is_some()
}
