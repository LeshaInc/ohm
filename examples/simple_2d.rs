use std::path::PathBuf;
use std::sync::Arc;

use ohm::asset::FileAssetSource;
use ohm::math::{vec2, Affine2, URect, UVec2, Vec2};
use ohm::renderer::SurfaceId;
use ohm::text::{FontFamilies, FontFamily, LineHeight, TextAlign, TextAttrs, TextBuffer};
use ohm::texture::MipmapMode;
use ohm::{Color, Encoder, EncoderScratch, Graphics, PathBuilder, Shadow};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

fn paint(encoder: &mut Encoder, size: Vec2, text_buffer: &mut TextBuffer) {
    encoder.rect(vec2(0.0, 0.0), size).color(Color::WHITE);

    encoder
        .rect(vec2(50.0, 50.0), size - vec2(100.0, 100.0))
        .color(Color::TRANSPAENT)
        .border(Color::rgb(1.0, 0.0, 0.0), 1.0);

    text_buffer.set_max_width(size.x - 100.0);
    text_buffer.compute_layout(encoder.font_db, encoder.text_shaper);
    encoder.text(vec2(50.0, 50.0), text_buffer);

    {
        let mut layer = encoder
            .layer()
            .tint(Color::rgba(0.5, 0.5, 0.5, 0.5))
            .transform(Affine2::from_scale_angle_translation(
                vec2(2.0, 2.0),
                30f32.to_radians(),
                vec2(600.0, -200.0),
            ));

        let shadow = Shadow {
            blur_radius: 0.0,
            spread_radius: 0.0,
            offset: vec2(8.0, 4.0),
            color: Color::rgba(0.0, 0.0, 0.0, 0.5),
        };

        layer
            .rect(vec2(80.0, 80.0), vec2(100.0, 100.0))
            .color(Color::rgb(1.0, 0.0, 0.0))
            .border(Color::BLACK, 2.0)
            .corner_radii(8.0)
            .shadow(shadow);

        layer
            .rect(vec2(120.0, 120.0), vec2(100.0, 100.0))
            .color(Color::rgb(0.0, 1.0, 0.0))
            .border(Color::BLACK, 2.0)
            .corner_radii(8.0)
            .shadow(shadow);

        let mut path = PathBuilder::new();
        path.move_to(vec2(0.0, 0.0));
        path.line_to(vec2(300.0, 100.0));
        path.line_to(vec2(50.0, 120.0));
        path.close();
        let path = path.finish();

        layer
            .fill_path(vec2(0.0, 190.0), &path)
            .color(Color::rgba(0.0, 0.0, 0.5, 0.5));
        layer
            .stroke_path(vec2(0.0, 190.0), &path)
            .color(Color::BLACK);
    }

    let shadow = Shadow {
        blur_radius: 12.0,
        spread_radius: 0.0,
        offset: vec2(0.0, 4.0),
        color: Color::rgba(0.0, 0.0, 0.0, 0.5),
    };

    encoder
        .rect(vec2(80.0, 80.0), vec2(100.0, 100.0))
        .color(Color::rgba(0.5, 0.0, 0.0, 0.5))
        .border(Color::BLACK, 2.0)
        .corner_radii(8.0)
        .shadow(shadow);

    encoder
        .rect(vec2(120.0, 120.0), vec2(100.0, 100.0))
        .color(Color::rgba(0.0, 0.5, 0.0, 0.5))
        .border(Color::BLACK, 2.0)
        .corner_radii(8.0)
        .shadow(shadow);

    encoder
        .rect(vec2(100.0, 400.0), vec2(574.0, 432.0))
        .image_path("file:kitten.jpg")
        .corner_radii(16.0);

    let image = encoder
        .texture_cache
        .add_image_from_path("file:kitten.jpg", MipmapMode::Enabled);

    encoder
        .rect(vec2(800.0, 700.0), vec2(100.0, 100.0))
        .image(&image)
        .image_clip_rect(URect::new(UVec2::new(200, 200), UVec2::new(300, 300)))
        .corner_radii(16.0)
        .border(Color::BLACK, 2.0);
}

struct AppState {
    window: Arc<Window>,
    graphics: Graphics,
    encoder_scratch: EncoderScratch,
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
                    .with_title("ohm example")
                    .with_inner_size(PhysicalSize::new(800, 600))
                    .with_transparent(true),
            )
            .map(Arc::new)
            .unwrap();

        let mut graphics = Graphics::new_wgpu();

        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("../../examples");
        graphics
            .asset_sources
            .add_source("file", FileAssetSource::new(path).unwrap());

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
            encoder_scratch: EncoderScratch::new(),
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
                    .resize_surface(state.surface, UVec2::new(new_size.width, new_size.height))
                    .unwrap();
                state.window.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                let size = state
                    .graphics
                    .renderer
                    .get_surface_size(state.surface)
                    .as_vec2();

                let mut encoder = state
                    .graphics
                    .create_encoder(&state.encoder_scratch, state.surface);

                paint(&mut encoder, size, &mut state.buffer);

                let draw_list = encoder.finish();

                state.graphics.render(&[draw_list]).unwrap();
                state.graphics.present().unwrap();
            }

            _ => {}
        }
    }
}

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.run_app(&mut App::default()).unwrap();
}
