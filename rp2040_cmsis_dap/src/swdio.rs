use crate::cmsis_dap::DapError;

#[derive(Clone, Copy)]
pub struct SwdIoConfig {
    pub clock_wait_cycles: u32,
    pub idle_cycles: u32,
    pub turn_around_cycles: u32,
    pub always_generate_data_phase: bool,
}

pub type SwdRequest = u8;

pub trait SwdIo {
    fn connect(&mut self);
    fn disconnect(&mut self);
    fn swj_clock(
        &mut self,
        config: &mut SwdIoConfig,
        frequency_hz: u32,
    ) -> core::result::Result<(), DapError>;
    fn swj_sequence(&mut self, config: &SwdIoConfig, count: usize, data: &[u8]);
    fn swd_read_sequence(&mut self, config: &SwdIoConfig, count: usize, data: &mut [u8]);
    fn swd_write_sequence(&mut self, config: &SwdIoConfig, count: usize, data: &[u8]);
    fn swd_transfer(
        &mut self,
        config: &SwdIoConfig,
        request: SwdRequest,
        data: u32,
    ) -> core::result::Result<u32, DapError>;
    fn enable_output(&mut self);
    fn disable_output(&mut self);
}