//! Semantic colors. Both palettes preserve existing screens during migration.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub background: u32,
    pub surface: u32,
    pub text: u32,
    pub muted: u32,
    pub border: u32,
    pub accent: u32,
    pub subtle: u32,
    pub error: u32,
    pub success: u32,
}

pub const LIGHT: Palette = Palette {
    background: 0xf4f6f8,
    surface: 0xffffff,
    text: 0x233348,
    muted: 0x677688,
    border: 0xe4e9ef,
    accent: 0x008e8c,
    subtle: 0xedf5f5,
    error: 0xc92f57,
    success: 0x007d78,
};

pub const COOL_LIGHT: Palette = Palette {
    background: 0xf3f5f7,
    surface: 0xffffff,
    text: 0x263549,
    muted: 0x697789,
    border: 0xdce3ea,
    accent: 0x008987,
    subtle: 0xe9f5f3,
    error: 0xc73655,
    success: 0x007d78,
};
