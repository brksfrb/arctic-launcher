//! The documented x64 THREADNAME_INFO notification, only for an owned encoder.
//! https://learn.microsoft.com/en-us/visualstudio/debugger/how-to-set-a-thread-name-in-native-code
use hudhook::windows::Win32::System::Diagnostics::Debug::EXCEPTION_DEBUG_INFO;

pub fn owned_encoder_thread_name(info: &EXCEPTION_DEBUG_INFO, owned_encoder: bool) -> bool {
    let record = &info.ExceptionRecord;
    // Windows can add the software-origin flag (0x80). Reject noncontinuable,
    // unwind, other exception flags, second-chance events, and other processes.
    if !owned_encoder
        || info.dwFirstChance == 0
        || record.ExceptionCode.0 as u32 != 0x406d1388
        || record.ExceptionFlags & !0x80 != 0
        || record.NumberParameters != 3
    {
        return false;
    }
    let p = &record.ExceptionInformation;
    // x64: DWORD type + padding, pointer, DWORD thread ID + DWORD reserved flags.
    p[0] as u32 == 0x1000
        && (0x10000..0x0000_8000_0000_0000).contains(&p[1])
        && p[2] as u32 != 0
        && p[2] >> 32 == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_owned_first_chance_thread_metadata_is_consumed() {
        let mut info = EXCEPTION_DEBUG_INFO {
            dwFirstChance: 1,
            ..Default::default()
        };
        info.ExceptionRecord.ExceptionCode.0 = 0x406d1388;
        info.ExceptionRecord.ExceptionFlags = 0x80;
        info.ExceptionRecord.NumberParameters = 3;
        info.ExceptionRecord.ExceptionInformation[..3].copy_from_slice(&[
            0x1234_0000_1000,
            0x12340000,
            42,
        ]);
        assert!(owned_encoder_thread_name(&info, true));
        assert!(!owned_encoder_thread_name(&info, false));
        info.dwFirstChance = 0;
        assert!(!owned_encoder_thread_name(&info, true));
        info.dwFirstChance = 1;
        for (index, value) in [(0, 0x1001), (1, 0), (2, 0), (2, 0x10000002a)] {
            let saved = info.ExceptionRecord.ExceptionInformation[index];
            info.ExceptionRecord.ExceptionInformation[index] = value;
            assert!(!owned_encoder_thread_name(&info, true));
            info.ExceptionRecord.ExceptionInformation[index] = saved;
        }
        info.ExceptionRecord.ExceptionFlags = 1;
        assert!(!owned_encoder_thread_name(&info, true));
        info.ExceptionRecord.ExceptionFlags = 0;
        assert!(owned_encoder_thread_name(&info, true));
        info.ExceptionRecord.ExceptionCode.0 = 0xc0000005u32 as i32;
        assert!(!owned_encoder_thread_name(&info, true));
    }
}
