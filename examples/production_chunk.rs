use qatq::{restore_production_chunk, try_encode_production_chunk, ProductionStorage, QatqError};

fn main() -> Result<(), QatqError> {
    let values: Vec<f32> = (0..1024)
        .map(|index| f32::from_bits(((index as u32) << 16) & 0x7fff_0000))
        .collect();

    let encoded = try_encode_production_chunk(&values)?;
    match encoded.metadata.storage {
        ProductionStorage::QatqExact => {
            assert_eq!(encoded.metadata.storage_label(), "qatq-exact");
        }
        ProductionStorage::RawF32LePassThrough => {
            assert_eq!(encoded.metadata.storage_label(), "raw-f32le-pass-through");
        }
    }

    let restored = restore_production_chunk(&encoded.metadata, encoded.stored_bytes())?;
    assert_eq!(f32_bits(&restored), f32_bits(&values));
    Ok(())
}

fn f32_bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|value| value.to_bits()).collect()
}
