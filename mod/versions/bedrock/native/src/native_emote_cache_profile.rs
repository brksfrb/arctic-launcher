//! Exact-family cache entry points and the actual edges establishing provenance.
//! Called only after world_hooks has accepted the complete executable image.
use super::{direct_return_rva, exact_function_candidate, function_witness};
use crate::native_scene_layout::{self as scene, EmoteCacheStorage};

#[derive(Clone, Copy)]
pub(crate) struct TreeCacheBinding {
    pub(crate) erase_rva: usize,
    pub(crate) lookup_rva: usize,
    pub(crate) fallback_predicate_return: usize,
    pub(crate) cached_predicate_return: usize,
    pub(crate) lookup_returns: [usize; 3],
}
impl TreeCacheBinding {
    pub(crate) fn erase_prefix(self) -> &'static [u8] {
        &[
            0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x40, 0x0f, 0x29, 0x74,
            0x24, 0x30,
        ]
    }
    pub(crate) fn lookup_prefix(self) -> &'static [u8] {
        &[
            0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83,
            0xec, 0x68, 0x48, 0x8d, 0x6c, 0x24, 0x60,
        ]
    }
}
#[derive(Clone, Copy)]
pub(crate) struct CacheMatrixInsertBinding {
    pub(crate) rva: usize,
    pub(crate) producer_return: usize,
}
impl CacheMatrixInsertBinding {
    pub(crate) fn prefix(self) -> &'static [u8] {
        &[
            0x41, 0x57, 0x41, 0x56, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x28,
        ]
    }
}
#[derive(Clone, Copy)]
pub(crate) struct CacheObserverBinding {
    pub(crate) producer_rva: usize,
    pub(crate) predicate_rva: usize,
    pub(crate) prepare_rva: usize,
    pub(crate) writer_return: usize,
    pub(crate) common_predicate_return: Option<usize>,
    pub(crate) tree: Option<TreeCacheBinding>,
    pub(crate) storage: EmoteCacheStorage,
    pub(crate) matrix_insert: Option<CacheMatrixInsertBinding>,
}
impl CacheObserverBinding {
    pub(crate) fn storage_name(self) -> &'static str {
        match self.storage {
            EmoteCacheStorage::Tree => "tree",
            EmoteCacheStorage::FlatParallelVectors => "flatParallelVectors",
        }
    }
    pub(crate) fn producer_prefix(self) -> &'static [u8] {
        match self.storage {
            EmoteCacheStorage::Tree => &[
                0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x48, 0x83,
                0xec, 0x78, 0x0f, 0x29, 0x74, 0x24, 0x60,
            ],
            EmoteCacheStorage::FlatParallelVectors => &[
                0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81,
                0xec, 0x08, 0x01, 0, 0, 0x48, 0x8d, 0xac, 0x24, 0x80, 0, 0, 0,
            ],
        }
    }
    pub(crate) fn prepare_prefix(self) -> &'static [u8] {
        match self.storage {
            EmoteCacheStorage::Tree => &[
                0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x55, 0x53, 0x48, 0x81,
                0xec, 0x58, 0x01, 0, 0,
            ],
            EmoteCacheStorage::FlatParallelVectors => &[
                0x55, 0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x81,
                0xec, 0x18, 0x01, 0, 0, 0x48, 0x8d, 0xac, 0x24, 0x80, 0, 0, 0,
            ],
        }
    }
    pub(crate) fn predicate_prefix(self) -> &'static [u8] {
        match self.storage {
            EmoteCacheStorage::Tree => &[
                0x41, 0x57, 0x41, 0x56, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec, 0x20, 0x4c, 0x8b, 0x89,
                0xd0, 0x01, 0, 0, 0x48, 0x8b, 0x81, 0xd8, 0x01, 0, 0,
            ],
            EmoteCacheStorage::FlatParallelVectors => &[
                0x41, 0x57, 0x41, 0x56, 0x41, 0x55, 0x41, 0x54, 0x56, 0x57, 0x53, 0x48, 0x83, 0xec,
                0x20, 0x4c, 0x8b, 0x91, 0xd0, 0x01, 0, 0, 0x4c, 0x8b, 0x89, 0xd8, 0x01, 0, 0,
            ],
        }
    }
}
pub(crate) fn cache_observer_binding() -> Result<CacheObserverBinding, String> {
    let (producer_rva, producer, _) = exact_function_candidate("emoteCacheProducer", 27)?;
    let (predicate_rva, predicate, _) = exact_function_candidate("emoteCachePredicate", 29)?;
    let (prepare_rva, prepare, _) = exact_function_candidate("emoteCachePrepare", 27)?;
    let storage = match prepare["storage"].as_str() {
        Some("tree") => EmoteCacheStorage::Tree,
        Some("flatParallelVectors") => EmoteCacheStorage::FlatParallelVectors,
        _ => return Err("unsupported native emote cache storage".into()),
    };
    if storage != scene::emote_cache::STORAGE
        || producer_rva == predicate_rva
        || producer_rva == prepare_rva
        || predicate_rva == prepare_rva
        || producer["abi"].as_str() != Some("partParamsBonesKeyGeometry5")
        || predicate["abi"].as_str() != Some("partParamsKeyByte3")
        || prepare["abi"].as_str() != Some("partParamsBonesKeyGeometryHistory6")
    {
        return Err("unsupported native emote cache ABI or scene family".into());
    }
    // The producer and preparer recurse with the original argument widths.
    // Predicate provenance independently joins the producer to its real callee.
    function_witness(&producer, producer_rva)?;
    function_witness(&prepare, prepare_rva)?;
    function_witness(&predicate, predicate_rva)?;
    let writer = super::palette_writer_binding()?;
    let writer_return = direct_return_rva(&prepare, "writerReturnRva", writer.rva)?;
    let (tree, common_predicate_return) = match storage {
        EmoteCacheStorage::Tree => {
            let (erase_rva, erase, _) = exact_function_candidate("emoteCacheErase", 16)?;
            let (lookup_rva, lookup, _) = exact_function_candidate("emoteCacheLookup", 21)?;
            if erase["abi"].as_str() != Some("cacheNode1")
                || lookup["abi"].as_str() != Some("cacheMapResultPart3")
            {
                return Err("unsupported native tree cache ABI".into());
            }
            if [producer_rva, predicate_rva, prepare_rva].contains(&erase_rva)
                || [producer_rva, predicate_rva, prepare_rva, erase_rva].contains(&lookup_rva)
            {
                return Err("native cache entries alias".into());
            }
            function_witness(&erase, erase_rva)?;
            function_witness(&lookup, lookup_rva)?;
            let fallback_predicate_return =
                direct_return_rva(&prepare, "fallbackPredicateReturnRva", predicate_rva)?;
            let cached_predicate_return =
                direct_return_rva(&prepare, "cachedPredicateReturnRva", predicate_rva)?;
            if fallback_predicate_return == cached_predicate_return {
                return Err("tree cache predicate branches alias".into());
            }
            let values = prepare["lookupReturnRvas"]
                .as_array()
                .filter(|values| values.len() == 3)
                .ok_or("tree cache requires three ordered lookup edges")?;
            let mut lookup_returns = [0usize; 3];
            for (i, value) in values.iter().enumerate() {
                let site = value
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or("invalid tree cache lookup return")?;
                let witness = serde_json::json!({"returnRva":site});
                lookup_returns[i] = direct_return_rva(&witness, "returnRva", lookup_rva)?;
                if i > 0 && lookup_returns[i] <= lookup_returns[i - 1] {
                    return Err("unordered native tree cache lookup edges".into());
                }
            }
            (
                Some(TreeCacheBinding {
                    erase_rva,
                    lookup_rva,
                    fallback_predicate_return,
                    cached_predicate_return,
                    lookup_returns,
                }),
                None,
            )
        }
        EmoteCacheStorage::FlatParallelVectors => {
            if scene::PRODUCT_VERSION != "1.26.60.5"
                || prepare["lookupReturnRvas"]
                    .as_array()
                    .is_none_or(|values| !values.is_empty())
            {
                return Err("unsupported flat cache family/lookup layout".into());
            }
            (
                None,
                Some(direct_return_rva(
                    &prepare,
                    "predicateReturnRva",
                    predicate_rva,
                )?),
            )
        }
    };
    let matrix_insert = if storage == EmoteCacheStorage::FlatParallelVectors {
        let (rva, insert, _) = exact_function_candidate("emoteCacheMatrixInsert", 13)?;
        if insert["abi"].as_str() != Some("vectorHeaderInsertionPointer2ReturnsPointer")
            || [producer_rva, predicate_rva, prepare_rva].contains(&rva)
        {
            return Err("unsupported cache matrix insertion ABI".into());
        }
        function_witness(&insert, rva)?;
        let site = insert["witnessRva"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|n| n.checked_add(5))
            .ok_or("cache matrix insertion return overflow")?;
        let witness = serde_json::json!({"returnRva":site});
        let producer_return = direct_return_rva(&witness, "returnRva", rva)?;
        Some(CacheMatrixInsertBinding {
            rva,
            producer_return,
        })
    } else {
        None
    };
    Ok(CacheObserverBinding {
        producer_rva,
        predicate_rva,
        prepare_rva,
        writer_return,
        common_predicate_return,
        tree,
        storage,
        matrix_insert,
    })
}
