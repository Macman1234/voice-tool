//! # ADC FIFO Example
//!
//! This application demonstrates how to read ADC samples in free-running mode,
//! and reading them from the FIFO by polling the fifo's `len()`.
//!
//! It may need to be adapted to your particular board layout and/or pin assignment.
//!
//! See the `Cargo.toml` file for Copyright and license details.

#![no_std]
#![no_main]

// Ensure we halt the program on panic (if we don't mention this crate it won't
// be linked)
use panic_halt as _;

// Alias for our HAL crate
use rp_pico::hal as hal;

// Some traits we need
use hal::Clock;

// GPIO traits
use embedded_hal::PwmPin;

// A shorter alias for the Peripheral Access Crate, which provides low-level
// register access
use hal::pac;

/// External high-speed crystal on the Raspberry Pi Pico board is 12 MHz. Adjust
/// if your board has a different frequency
const XTAL_FREQ_HZ: u32 = 12_000_000u32;

/// Entry point to our bare-metal application.
///
/// The `#[rp2040_hal::entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables and the spinlock are initialised.
///
/// The function configures the RP2040 peripherals, then prints the temperature
/// in an infinite loop.
#[hal::entry]
fn main() -> ! {
    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    let clocks = hal::clocks::init_clocks_and_plls(
        XTAL_FREQ_HZ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    ).ok().unwrap();

    // The delay object lets us wait for specified amounts of time (in
    // milliseconds)
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    // The single-cycle I/O block controls our GPIO pins
    let sio = hal::Sio::new(pac.SIO);

    // Set the pins to their default state
    let pins = rp_pico::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Init PWMs
    let mut pwm_slices = hal::pwm::Slices::new(pac.PWM, &mut pac.RESETS);

    // Configure PWM4
    let pwm = &mut pwm_slices.pwm4;
    pwm.set_ph_correct();
    pwm.enable();

    // Output channel B on PWM4 to the LED pin
    let channel = &mut pwm.channel_b;
    channel.output_to(pins.led);

    // Enable ADC
    let mut adc = hal::Adc::new(pac.ADC, &mut pac.RESETS);

    // Configure GPIO26 as an ADC input
    let mut adc_pin_1 = hal::adc::AdcPin::new(pins.gpio27.into_floating_input());

    // Configure free-running mode:
    let mut adc_fifo = adc
        .build_fifo()
        // Set clock divider to target a sample rate of 1000 samples per second (1ksps).
        // The value was calculated by `(48MHz / 1ksps) - 1 = 47999.0`.
        // Please check the `clock_divider` method documentation for details.
        .clock_divider(4799, 0)
        // sample the temperature sensor first
        .set_channel(&mut adc_pin_1)
        // Uncomment this line to produce 8-bit samples, instead of 12 bit (lower bits are discarded)
        //.shift_8bit()
        // start sampling
        .start();

    // intialize 16kb long buffer
    let mut window: [u16; 105] = [0; 105];
    // by default, use the whole thing
    let mut windowlen = 100;
    let mut i = 0;

    loop {
        // get number of unloaded samples, and load that many samples in to the rolling window
        let sample_len= adc_fifo.len();
        if adc_fifo.len() > 0 {
            for n in 0..sample_len {
                window[i] = adc_fifo.read();
                i += 1;
                if i > windowlen {
                    i = 0;
                }
            }
        }
        // average window
        let mut sum: usize = 0;
        for s in &window[0..windowlen] {
            sum = sum + usize::from(*s);
        }
        let average: u16 = (sum / windowlen).try_into().unwrap();
        // scale value to pwm range
        let duty = average * (25000/4096);
        // output pwm
        channel.set_duty(duty);
    }
}

// End of file