#![no_main]

use libfuzzer_sys::fuzz_target;
use qatq::TensorDType;

fuzz_target!(|data: &[u8]| {
    let Some((&selector, payload)) = data.split_first() else {
        return;
    };
    let dtype = if selector & 1 == 0 {
        TensorDType::F16
    } else {
        TensorDType::BF16
    };
    let byte_len = payload.len().min(8_192) & !1;
    let source = &payload[..byte_len];
    if let Ok(encoded) = qatq::try_encode_qatq_exact_tensor_le(source, dtype) {
        let decoded = qatq::decode_qatq_exact_tensor_le(&encoded)
            .expect("encoder output must decode exactly");
        assert_eq!(decoded.dtype, dtype);
        assert_eq!(decoded.bytes_le, source);
    }
});
