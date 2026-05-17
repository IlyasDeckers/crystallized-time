use crate::quantizer::Scale;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QuantizeConfig {
    pub scale: Scale,
    pub root_note: u8,
}

impl Default for QuantizeConfig {
    fn default() -> Self {
        Self {
            scale: Scale::Unquantized,
            root_note: 60,
        }
    }
}
