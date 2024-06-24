use std::sync::Arc;

use ohm2d::math::{vec2, UVec2};
use ohm2d::text::{FontFamilies, FontFamily, LineHeight, TextAlign, TextAttrs, TextBuffer};
use ohm2d::{
    Border, Color, Command, CornerRadii, DrawGlyph, DrawLayer, DrawList, DrawRect, Fill, Graphics,
    Renderer, Shadow,
};
use ohm2d_core::renderer::SurfaceId;
use ohm2d_wgpu::WgpuRenderer;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

struct AppState {
    window: Arc<Window>,
    graphics: Graphics<WgpuRenderer>,
    buffer: TextBuffer,
    surface: SurfaceId,
}

#[derive(Default)]
struct App {
    state: Option<AppState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_title("ohm2d example")
                    .with_inner_size(PhysicalSize::new(800, 600)),
            )
            .map(Arc::new)
            .unwrap();

        let mut graphics = Graphics::new_wgpu();
        let surface = graphics
            .renderer
            .create_surface(window.clone(), UVec2::new(800, 600))
            .unwrap();

        let mut buffer = TextBuffer::new();

        let attrs = TextAttrs {
            size: 14.0,
            line_height: LineHeight::Relative(1.3),
            align: TextAlign::Justify,
            fonts: FontFamilies::new(FontFamily::new("Open Sans"))
                .with(FontFamily::new("Noto Color Emoji"))
                .with(FontFamily::new("DejaVu Sans")),
            ..Default::default()
        };

        buffer.push(
            attrs.clone(),
            "This 👭🎵🌘 also 😋🚣‍♂️🙇‍♂️ supports 🚥 emoji! 🚈🧤🩸♦︎😛🐕👨‍🦲⛷💫👡🏮🍷♗📽🌵➗🎄🕟 👢☄️👨‍🔧 Isn't it 🗻🍡 neat? 🦋👨‍🦯📕🎐🏩💙🚵‍♀️\n\nLorem ipsum dolor sit amet, eam ad fugit vocibus, quo autem consul definitionem ex, at sed melius appetere. Ne duis numquam fabulas his, sit etiam mediocritatem no, no nec diam possit scaevola. Dicta viris eirmod ius cu, elit scribentur id vim, mei et elitr iudicabit necessitatibus. Ius ad augue invidunt, ius cu paulo aliquam, id enim euismod contentiones eum. Cum an omnium consulatu scriptorem, te vim mundi copiosae.\n\n"
        );

        buffer.push(
            attrs.clone(),
            "يكن تحرير الأمم البرية قد. في فصل أراض الأمريكية, أن بأيدي تزامناً الموسوعة شيء. هذا قد الشتوية تزامناً, ان يكن يقوم كنقطة الدنمارك, الشرقي الطريق باستخدام دنو ثم. كل نهاية العالمية سنغافورة قام, من نفس حاول مكثّفة الشرقية. أن فقد وبغطاء الإمتعاض الإقتصادية, بـ تُصب قِبل اكتوبر دار. ذلك في تجهيز النفط الإقتصادية.\n\n",
        );

        buffer.push(
            attrs,
            "אם היא אודות ספרדית משפטים, או פנאי קהילה אתה, ספורט מיזמים אל שמו. כתב יוני למנוע העזרה של, אחד או הבהרה המקושרים, אל ואמנות רומנית ותשובות שמו.\n"
        );

        self.state = Some(AppState {
            window,
            graphics,
            buffer,
            surface,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                state
                    .graphics
                    .renderer
                    .resize_surface(state.surface, UVec2::new(new_size.width, new_size.height));
                state.window.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                let size = state
                    .graphics
                    .renderer
                    .get_surface_size(state.surface)
                    .as_vec2();

                let mut commands = Vec::new();

                commands.push(Command::DrawRect(DrawRect {
                    pos: vec2(0.0, 0.0),
                    size,
                    fill: Fill::Solid(Color::WHITE),
                    corner_radii: CornerRadii::default(),
                    border: None,
                    shadow: None,
                }));

                commands.push(Command::DrawRect(DrawRect {
                    pos: vec2(50.0, 50.0),
                    size: size - vec2(100.0, 100.0),
                    fill: Fill::Solid(Color::TRANSPAENT),
                    corner_radii: CornerRadii::default(),
                    border: Some(Border {
                        color: Color::rgb(1.0, 0.0, 0.0),
                        width: 1.0,
                    }),
                    shadow: None,
                }));

                state.buffer.set_max_width(size.x - 100.0);
                state.buffer.compute_layout(
                    &mut state.graphics.font_db,
                    &mut *state.graphics.text_shaper,
                );

                for run in state.buffer.runs() {
                    let mut pos = run.pos + vec2(50.0, 50.0);
                    for glyph in &state.buffer.glyphs()[run.glyph_range.clone()] {
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

                let mut layer_commands = Vec::new();

                let shadow = Some(Shadow {
                    blur_radius: 12.0,
                    spread_radius: 0.0,
                    offset: vec2(0.0, 4.0),
                    color: Color::rgba(0.0, 0.0, 0.0, 1.0),
                });

                layer_commands.push(Command::DrawRect(DrawRect {
                    pos: vec2(80.0, 80.0),
                    size: vec2(100.0, 100.0),
                    fill: Fill::Solid(Color::rgb(1.0, 0.0, 0.0)),
                    corner_radii: CornerRadii::new_equal(8.0),
                    border: None,
                    shadow,
                }));

                layer_commands.push(Command::DrawRect(DrawRect {
                    pos: vec2(120.0, 120.0),
                    size: vec2(100.0, 100.0),
                    fill: Fill::Solid(Color::rgb(0.0, 1.0, 0.0)),
                    corner_radii: CornerRadii::new_equal(8.0),
                    border: None,
                    shadow,
                }));

                commands.push(Command::DrawLayer(DrawLayer {
                    commands: &layer_commands,
                    tint: Color::rgba(0.5, 0.5, 0.5, 0.5),
                    scissor: None,
                }));

                let shadow = Some(Shadow {
                    blur_radius: 12.0,
                    spread_radius: 0.0,
                    offset: vec2(0.0, 4.0),
                    color: Color::rgba(0.0, 0.0, 0.0, 0.5),
                });

                commands.push(Command::DrawRect(DrawRect {
                    pos: vec2(280.0, 80.0),
                    size: vec2(100.0, 100.0),
                    fill: Fill::Solid(Color::rgba(0.5, 0.0, 0.0, 0.5)),
                    corner_radii: CornerRadii::new_equal(8.0),
                    border: None,
                    shadow,
                }));

                commands.push(Command::DrawRect(DrawRect {
                    pos: vec2(320.0, 120.0),
                    size: vec2(100.0, 100.0),
                    fill: Fill::Solid(Color::rgba(0.0, 0.5, 0.0, 0.5)),
                    corner_radii: CornerRadii::new_equal(8.0),
                    border: None,
                    shadow,
                }));

                state
                    .graphics
                    .render(&[DrawList {
                        surface: state.surface,
                        commands: &commands,
                    }])
                    .unwrap();
                state.graphics.present();
            }

            _ => {}
        }
    }
}

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.run_app(&mut App::default()).unwrap();
}
