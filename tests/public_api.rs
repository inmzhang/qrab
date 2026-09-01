use qrab::{BackendEscapes, EscapeBlock, MeasurementShape};

#[test]
fn exports_types_used_by_public_ast_fields() {
    let _: BackendEscapes = BackendEscapes::default();
    let _: EscapeBlock = EscapeBlock::default();
    let _: MeasurementShape = MeasurementShape::D;
}
