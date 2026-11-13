//! Draw-boundary observations do not depend on an out-of-line rotation helper.
//! Bindings still require the selected complete image and scene layout family.
use super::{exact_function_candidate, function_witness};
use crate::native_scene_layout as scene;

#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TransformObservation {
    #[default]
    HelperThenDraw,
    DrawEntry,
}
#[derive(Clone, Copy)]
pub(crate) struct TransformBinding {
    pub(crate) draw_rva: usize,
    pub(crate) renderer_rva: usize,
    pub(crate) table_rva: usize,
    pub(crate) global_rva: Option<usize>,
    pub(crate) observation: TransformObservation,
    stack_bytes: usize,
}
impl TransformBinding {
    pub(crate) fn draw_prefix(self) -> &'static [u8] {
        match self.stack_bytes {
            0x268 => &[
                0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81,
                0xec, 0x68, 0x02, 0, 0, 0x48, 0x8d, 0xac, 0x24, 0x80, 0, 0, 0,
            ],
            0x248 => &[
                0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81,
                0xec, 0x48, 0x02, 0, 0, 0x48, 0x8d, 0xac, 0x24, 0x80, 0, 0, 0,
            ],
            _ => &[],
        }
    }
    pub(crate) fn global_prefix(self) -> &'static [u8] {
        &[
            0x56, 0x57, 0x53, 0x48, 0x81, 0xec, 0x10, 0x01, 0, 0, 0x44, 0x0f, 0x29, 0xb4, 0x24, 0,
            0x01, 0, 0,
        ]
    }
}
pub(crate) fn transform_binding() -> Result<TransformBinding, String> {
    let (draw_rva, draw, image_size) = exact_function_candidate("nativeModelDraw", 27)?;
    let observation = match draw["observation"].as_str() {
        Some("helperThenDraw") => TransformObservation::HelperThenDraw,
        Some("drawEntry") => TransformObservation::DrawEntry,
        _ => return Err("unsupported model transform observation strategy".into()),
    };
    let stack_bytes = draw["stackBytes"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("missing native model draw frame")?;
    if draw["abi"].as_str() != Some("fivePointerVoid")
        || draw["matrixStackLayout"].as_str() != Some(scene::matrix_stacks::LAYOUT)
        || draw["rendererSlot"] != 2
        || stack_bytes != scene::model_renderer::DRAW_STACK_BYTES
        || draw["observation"].as_str() != Some(scene::model_renderer::OBSERVATION)
    {
        return Err("native draw ABI or scene strategy differs".into());
    }
    let renderer_rva = draw["rendererRva"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n == scene::model_renderer::RENDER_RVA)
        .ok_or("native renderer entry differs")?;
    let table_rva = draw["rendererVtableRva"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n == scene::model_renderer::VTABLE)
        .ok_or("native renderer table differs")?;
    let &(base, start, end) = super::CODE_IMAGE.get().ok_or("native image unavailable")?;
    let _site = draw["witnessRva"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| renderer_rva <= *n && *n < end)
        .ok_or("native draw caller differs")?;
    if renderer_rva < start
        || renderer_rva >= end
        || draw_rva == renderer_rva
        || table_rva
            .checked_add(24)
            .is_none_or(|last| last > image_size)
        || super::pointer(
            base.checked_add(table_rva)
                .and_then(|n| n.checked_add(16))
                .ok_or("native renderer slot overflow")?,
        ) != base.checked_add(renderer_rva)
    {
        return Err("native renderer table slot differs".into());
    }
    function_witness(&draw, draw_rva)?;
    let global_rva = match observation {
        TransformObservation::HelperThenDraw => {
            if scene::PRODUCT_VERSION != "1.26.5203.0" {
                return Err("global helper strategy lacks this family".into());
            }
            let (rva, global, _) = exact_function_candidate("nativeGlobalTransform", 19)?;
            if global["abi"].as_str() != Some("dataMatrixContext3")
                || rva == draw_rva
                || rva == renderer_rva
            {
                return Err("native global transform ABI differs".into());
            }
            function_witness(&global, rva)?;
            Some(rva)
        }
        TransformObservation::DrawEntry => {
            if scene::PRODUCT_VERSION != "1.26.60.5" {
                return Err("draw-entry strategy lacks this family".into());
            }
            None
        }
    };
    Ok(TransformBinding {
        draw_rva,
        renderer_rva,
        table_rva,
        global_rva,
        observation,
        stack_bytes,
    })
}
