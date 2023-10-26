use anyhow::Result;
use ohm2d::math::{vec2, UVec2};
use ohm2d::text::{FontFamilies, FontFamily, TextAlign, TextAttrs, TextBuffer};
use ohm2d::{
    Border, Color, Command, CornerRadii, DrawGlyph, DrawList, DrawRect, Fill, Graphics, Renderer,
};
use winit::dpi::PhysicalSize;
use winit::event::{Event, WindowEvent};
use winit::window::WindowBuilder;

fn main() -> Result<()> {
    let event_loop = winit::event_loop::EventLoop::new()?;
    let window = WindowBuilder::new()
        .with_title("ohm2d example")
        .with_inner_size(PhysicalSize::new(800, 600))
        .build(&event_loop)?;

    let mut graphics = Graphics::new_wgpu();
    let surface = unsafe {
        graphics
            .renderer
            .create_surface(&window, UVec2::new(800, 600))?
    };

    let mut buffer = TextBuffer::new();

    let attrs = TextAttrs {
        size: 20.0,
        align: TextAlign::Right,
        fonts: FontFamilies::new(FontFamily::new("Open Sans"))
            .add(FontFamily::new("Noto Color Emoji"))
            .add(FontFamily::new("Noto Sans Symbols 2")),
        ..Default::default()
    };

    buffer.push(
        attrs,
        "Lorem ipsum dolor sit amet,\n\nThis 👭🎵🌘 also 😋🚣‍♂️🙇‍♂️ supports 🚥 emoji! 🚈🧤🩸♦︎😛🐕👨‍🦲⛷💫👡🏮🍷♗📽🌵➗🎄🕟 👢☄️👨‍🔧 Isn't it 🗻🍡 neat? 🦋👨‍🦯📕🎐🏩💙🚵‍♀️\n\nLorem ipsum... consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Accumsan lacus vel facilisis volutpat est velit egestas dui id. Leo duis ut diam quam nulla porttitor. Odio ut enim blandit volutpat maecenas. Amet mattis vulputate enim nulla aliquet porttitor lacus luctus accumsan. Dignissim suspendisse in est ante in nibh mauris cursus. Fermentum iaculis eu non diam phasellus vestibulum lorem sed risus. Dapibus ultrices in iaculis nunc sed augue. Vel risus commodo viverra maecenas accumsan lacus. Sed id semper risus in hendrerit gravida rutrum quisque. Id nibh tortor id aliquet lectus proin nibh. Ipsum a arcu cursus vitae congue mauris. Pellentesque id nibh tortor id aliquet lectus proin nibh. Sociis natoque penatibus et magnis dis parturient montes nascetur. Lacinia at quis risus sed vulputate odio. Id diam vel quam elementum pulvinar etiam non quam lacus. Tristique senectus et netus et malesuada fames."
    );

    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => elwt.exit(),

        Event::WindowEvent {
            event: WindowEvent::Resized(new_size),
            ..
        } => {
            graphics
                .renderer
                .resize_surface(surface, UVec2::new(new_size.width, new_size.height));
            window.request_redraw();
        }

        Event::WindowEvent {
            event: WindowEvent::RedrawRequested,
            ..
        } => {
            let size = graphics.renderer.get_surface_size(surface);

            let mut commands = Vec::new();
            commands.push(Command::Clear(Color::WHITE));

            commands.push(Command::DrawRect(DrawRect {
                pos: vec2(50.0, 50.0),
                size: size.as_vec2() - vec2(100.0, 100.0),
                fill: Fill::Solid(Color::TRANSPAENT),
                corner_radii: CornerRadii::default(),
                border: Some(Border {
                    color: Color::rgb(1.0, 0.0, 0.0),
                    width: 1.0,
                }),
                shadow: None,
            }));

            buffer.set_max_width(size.x as f32 - 100.0);
            buffer.compute_layout(&mut graphics.font_db, &mut *graphics.text_shaper);

            for run in buffer.runs() {
                let mut pos = run.pos + vec2(50.0, 50.0);
                for glyph in &buffer.glyphs()[run.glyph_range.clone()] {
                    commands.push(Command::DrawGlyph(DrawGlyph {
                        pos: pos + glyph.offset,
                        size: run.font_size,
                        font: run.font,
                        glyph: glyph.glyph_id,
                        color: Color::BLACK,
                    }));
                    pos.x += glyph.x_advance;
                }
            }

            graphics
                .render(&[DrawList {
                    surface,
                    commands: &commands,
                }])
                .unwrap();

            graphics.present();
        }

        _ => {}
    })?;

    Ok(())
}
