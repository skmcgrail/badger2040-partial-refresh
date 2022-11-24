#![no_std]
#![no_main]
#![allow(clippy::cast_possible_wrap)]

use bsp::entry;
#[allow(unused_imports)]
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::v2::OutputPin;
use fugit::{HertzU32, MicrosDurationU32};
use panic_probe as _;

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
use pimoroni_badger2040 as bsp;

use bsp::hal::clocks::Clock;

// Bring in all the rest of our dependencies from the BSP
use bsp::hal;
use bsp::pac;
use embedded_graphics::mono_font::ascii::FONT_4X6;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::{Point, Size};
use embedded_graphics::primitives::{Primitive, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::Text;
use embedded_graphics::Drawable;
use embedded_hal::timer::CountDown;
use uc8151::{Uc8151, HEIGHT, WIDTH};

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let cp = pac::CorePeripherals::take().unwrap();

    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    let clocks = hal::clocks::init_clocks_and_plls(
        pimoroni_badger2040::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let sio = hal::Sio::new(pac.SIO);

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let mut power = pins.p3v3_en.into_push_pull_output();
    let _ = power.set_high();

    let timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS);

    let mut count_down = timer.count_down();

    // Set up the pins for the e-ink display
    let _ = pins.sclk.into_mode::<hal::gpio::FunctionSpi>();
    let _ = pins.mosi.into_mode::<hal::gpio::FunctionSpi>();
    let spi = hal::Spi::<_, _, 8>::new(pac.SPI0);
    let dc = pins.inky_dc.into_push_pull_output();
    let cs = pins.inky_cs_gpio.into_push_pull_output();
    let busy = pins.inky_busy.into_pull_up_input();
    let reset = pins.inky_res.into_push_pull_output();

    let spi = spi.init(
        &mut pac.RESETS,
        &clocks.peripheral_clock,
        HertzU32::Hz(1_000_000),
        &embedded_hal::spi::MODE_0,
    );

    let mut display = Uc8151::new(spi, cs, dc, busy, reset);

    // Reset display
    display.disable();
    count_down.start(MicrosDurationU32::micros(10));
    let _ = nb::block!(count_down.wait());
    display.enable();
    count_down.start(MicrosDurationU32::micros(10));
    let _ = nb::block!(count_down.wait());
    // Wait for the screen to finish reset
    while display.is_busy() {}

    let mut delay = cortex_m::delay::Delay::new(cp.SYST, clocks.system_clock.freq().to_Hz());

    // Initialise display. Using the default LUT speed setting
    display.setup(&mut delay, uc8151::LUT::Internal).unwrap();

    let mut current = 0i32;
    let mut channel = 0;

    display.update().unwrap();

    loop {
        let bounds = Rectangle::new(Point::new(0, current), Size::new(WIDTH, 8));

        bounds
            .into_styled(
                PrimitiveStyleBuilder::default()
                    .stroke_color(BinaryColor::Off)
                    .fill_color(BinaryColor::On)
                    .stroke_width(1)
                    .build(),
            )
            .draw(&mut display)
            .unwrap();

        Text::new(
            value_text(channel),
            bounds.center() + Point::new(0, 2),
            MonoTextStyle::new(&FONT_4X6, BinaryColor::Off),
        )
        .draw(&mut display)
        .unwrap();

        display.partial_update(bounds.try_into().unwrap()).unwrap();

        current = (current + 8) % HEIGHT as i32;
        channel = (channel + 1) % 16;

        count_down.start(MicrosDurationU32::millis(50));
        let _ = nb::block!(count_down.wait());
    }
}

fn value_text(value: i32) -> &'static str {
    const CHANNEL_NUM: &[&str] = &[
        "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
    ];

    #[allow(clippy::cast_sign_loss)]
    CHANNEL_NUM[(value % 16) as usize]
}
