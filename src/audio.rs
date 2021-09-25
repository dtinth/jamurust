use std::os::raw::c_int;

mod opus_custom {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    #![allow(deref_nullptr)]
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub struct Decoder {
    decoder: *mut opus_custom::OpusCustomDecoder,
    mode: *mut opus_custom::OpusCustomMode,
}

impl Decoder {
    pub fn new() -> Decoder {
        Self::new_with_custom_params(48000, 2, 128)
    }
    pub fn new_with_custom_params(sample_rate: u32, channels: u8, frame_size: u32) -> Decoder {
        unsafe {
            let mut err: c_int = 0;
            let mode = opus_custom::opus_custom_mode_create(
                sample_rate as c_int,
                frame_size as c_int,
                &mut err,
            );
            if mode.is_null() {
                panic!("opus_custom_mode_create failed: {}", err);
            }
            let decoder =
                opus_custom::opus_custom_decoder_create(mode, channels as c_int, &mut err);
            if decoder.is_null() {
                panic!("opus_custom_decoder_create failed: {}", err);
            }
            Decoder { decoder, mode }
        }
    }
    pub fn decode(&self, packet: &[u8], buffer: &mut [i16]) -> usize {
        unsafe {
            opus_custom::opus_custom_decode(
                self.decoder,
                packet.as_ptr(),
                packet.len() as c_int,
                buffer.as_mut_ptr(),
                (buffer.len() / 2) as c_int,
            ) as usize
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            opus_custom::opus_custom_decoder_destroy(self.decoder);
            opus_custom::opus_custom_mode_destroy(self.mode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_opus_custom_bindings() {
        let decoder = Decoder::new(48000, 2, 128);
        let mut buffer: [i16; 960] = [0; 960];
        let mut packet = [0u8; 165];
        packet[0] = 0x04;
        packet[1] = 0xff;
        packet[2] = 0xfe;
        let decoded = decoder.decode(&packet, &mut buffer);
        assert_eq!(decoded, 128);
        for sample in buffer {
            assert_eq!(sample, 0);
        }
    }
}
