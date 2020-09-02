const CONTROL_CHANGE: u8 = 0b1011;

pub fn rpn_message(
    channel: u8,
    parameter_number_msb: u8,
    parameter_number_lsb: u8,
    value: u8,
) -> [[u8; 3]; 3] {
    let control_change = channel_message(CONTROL_CHANGE, channel);
    [
        [control_change, 0x65, parameter_number_msb],
        [control_change, 0x64, parameter_number_lsb],
        [control_change, 0x06, value],
    ]
}

fn channel_message(prefix: u8, channel_nr: u8) -> u8 {
    prefix << 4 | channel_nr
}
