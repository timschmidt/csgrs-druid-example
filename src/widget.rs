use crate::graphics::{draw_line, draw_triangle};
use crate::math::{calculate_normal, multiply_matrices, multiply_matrix_vector, point_in_triangle};
use crate::state::AppState;
use crate::vertex::Vertex;
use druid::kurbo::Point;
use druid::text::FontFamily;
use druid::widget::prelude::*;
use druid::widget::{Controller, ControllerHost, Label};
use druid::WidgetExt;
use druid::{
    commands,
    piet::{InterpolationMode, Text, TextLayout, TextLayoutBuilder},
    Color, RenderContext, Widget, WindowDesc,
};
use std::time::Instant;
use csgrs::csg::CSG;
use csgrs::vertex::Vertex as CSGVertex;

/// 3D cube widget
pub struct CSGWidget {
    csg: CSG::<()>,
    frames_since_last_update: usize,
    last_fps_calculation: Instant,
    fps: f64,
    /// Is the user currently dragging for rotation?
    dragging_rotation: bool,
    /// Is the user currently dragging for translation?
    dragging_translation: bool,
    /// Last mouse position
    last_mouse_pos: Point,
    /// Widget size
    size: Size,
}

impl CSGWidget {
    pub fn new() -> Self {
        CSGWidget {
            csg: CSG::<()>::sphere(10.0, 16, 12, None),
            frames_since_last_update: 0,
            last_fps_calculation: Instant::now(),
            fps: 0.0,
            dragging_rotation: false,
            dragging_translation: false,
            last_mouse_pos: Point::ZERO,
            size: Size::ZERO,
        }
    }
    
    /// A helper to project a 3D point to 2D screen coordinates.
    fn project_point(&self, pos: &csgrs::vertex::Vertex) -> [f64; 2] {
        // For example, assume an orthographic projection where:
        //  - We ignore the z coordinate,
        //  - We center the object in the widget,
        //  - And we apply a uniform scale factor.
        let center = Point::new(self.size.width / 2.0, self.size.height / 2.0);
        // Here we assume a scale based on widget size and a zoom factor (say 1.0)
        let scale = (self.size.height.min(self.size.width) / 4.0) * 1.0;
        let screen_x = pos.pos.coords.x * scale + center.x;
        let screen_y = pos.pos.coords.y * scale + center.y;
        [screen_x, screen_y]
    }
}

impl Widget<AppState> for CSGWidget {
    /// Handle events for the cube widget
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppState, _env: &Env) {
        match event {
            Event::WindowConnected => {
                ctx.request_timer(std::time::Duration::from_millis(16));
                // Request focus to receive keyboard events
                ctx.request_focus();
            }
            Event::Timer(_) => {
                if !data.paused && !self.dragging_rotation && !self.dragging_translation {
                    data.angle_x += 0.01;
                    data.angle_y += 0.02;
                    ctx.request_paint();
                }
                ctx.request_timer(std::time::Duration::from_millis(16));
            }
            Event::KeyDown(key_event) => {
                if let druid::keyboard_types::Key::Character(s) = &key_event.key {
                    match s.as_str() {
                        "d" | "D" => {
                            data.debug = !data.debug;
                            ctx.request_paint();
                        }
                        "p" | "P" => {
                            data.paused = !data.paused;
                            // Reset any mouse events that were captured
                            self.last_mouse_pos = Point::ZERO;
                            self.dragging_rotation = false;
                            self.dragging_translation = false;
                            ctx.request_paint();
                        }
                        "q" | "Q" => {
                            // Submit the QUIT_APP command to exit the application
                            ctx.submit_command(commands::QUIT_APP);
                        }
                        "w" | "W" => {
                            if !data.paused {
                                data.wireframe = !data.wireframe;
                                ctx.request_paint();
                            }
                        }
                        "r" | "R" => {
                            if !data.paused {
                                // Reset to default values
                                data.angle_x = 0.0;
                                data.angle_y = 0.0;
                                data.translation = [0.0, 0.0];
                                data.zoom = 1.0;
                                data.wireframe = false;
                                ctx.request_paint();
                            }
                        }
                        "h" | "H" => {
                            let program_name = env!("CARGO_PKG_NAME");
                            let program_version = env!("CARGO_PKG_VERSION");
                            let program_authors = env!("CARGO_PKG_AUTHORS");

                            let controls_text: &[&str] = &[
                                "Controls:",
                                " - H: Open this help window",
                                " - Q: Quit the application",
                                " - D: Toggle debug mode",
                                " - P: Pause/unpause rotation",
                                " - W: Toggle wireframe mode",
                                " - R: Reset cube",
                                " - Mouse Left Drag: Rotate cube",
                                " - Mouse Right Drag: Translate cube",
                                " - Mouse Wheel: Zoom in/out",
                                "",
                            ];

                            // Assemble help text, putting multiple authors (if any) on separate lines
                            let mut help_text = String::new();
                            for line in controls_text {
                                help_text.push_str(line);
                                help_text.push('\n');
                            }
                            if !program_authors.is_empty() {
                                help_text.push_str(&format!("Author(s): {}\n", program_authors));
                            }
                            help_text.push_str(&format!("Version: {}\n", program_version));

                            struct CloseOnEsc;
                            impl<W: Widget<AppState>> Controller<AppState, W> for CloseOnEsc {
                                fn event(
                                    &mut self,
                                    child: &mut W,
                                    ctx: &mut EventCtx,
                                    event: &Event,
                                    data: &mut AppState,
                                    env: &Env,
                                ) {
                                    match event {
                                        Event::WindowConnected => {
                                            ctx.request_focus();
                                        }
                                        Event::KeyDown(key_event) => {
                                            if let druid::keyboard_types::Key::Escape =
                                                &key_event.key
                                            {
                                                ctx.window().close();
                                                return;
                                            }
                                        }
                                        _ => {}
                                    }
                                    child.event(ctx, event, data, env);
                                }

                                fn lifecycle(
                                    &mut self,
                                    child: &mut W,
                                    ctx: &mut LifeCycleCtx,
                                    event: &LifeCycle,
                                    data: &AppState,
                                    env: &Env,
                                ) {
                                    if let LifeCycle::WidgetAdded = event {
                                        ctx.register_for_focus();
                                    }
                                    child.lifecycle(ctx, event, data, env);
                                }
                            }

                            let help_widget = ControllerHost::new(
                                Label::new(help_text).with_text_size(14.0).padding(10.0),
                                CloseOnEsc,
                            );

                            let help_window = WindowDesc::new(help_widget)
                                .title(format!("About {}", program_name).to_string())
                                .resizable(false)
                                .window_size((450.0, 300.0));

                            ctx.new_window(help_window);
                        }
                        _ => {}
                    }
                }
            }
            Event::MouseDown(mouse_event) => {
                if !data.paused {
                    self.last_mouse_pos = mouse_event.pos;
                    // Compute projected vertices

                    // Define cube faces (each face is defined by 4 vertex indices)
                    
                    let mut clicked_inside_cube = false;
                    let _click_point = [mouse_event.pos.x, mouse_event.pos.y];

                    /*
                    for &(a, b, c, d) in &faces {
                        // Triangle 1: a, b, c
                        let v0 = &vertices_with_normals[a];
                        let v1 = &vertices_with_normals[b];
                        let v2 = &vertices_with_normals[c];
                        if point_in_triangle(
                            click_point,
                            v0.screen_position,
                            v1.screen_position,
                            v2.screen_position,
                        ) {
                            clicked_inside_cube = true;
                            break;
                        }
                        // Triangle 2: a, c, d
                        let v0 = &vertices_with_normals[a];
                        let v1 = &vertices_with_normals[c];
                        let v2 = &vertices_with_normals[d];
                        if point_in_triangle(
                            click_point,
                            v0.screen_position,
                            v1.screen_position,
                            v2.screen_position,
                        ) {
                            clicked_inside_cube = true;
                            break;
                        }
                    }
                    */

                    if clicked_inside_cube {
                        match mouse_event.button {
                            druid::MouseButton::Left => {
                                self.dragging_rotation = true;
                            }
                            druid::MouseButton::Right => {
                                self.dragging_translation = true;
                            }
                            _ => {}
                        }
                        ctx.set_active(true); // Capture mouse events
                    }
                }
            }
            Event::MouseMove(mouse_event) => {
                if !data.paused {
                    if self.dragging_rotation {
                        let delta = mouse_event.pos - self.last_mouse_pos;
                        // Update rotation angles based on mouse movement
                        data.angle_x += delta.y * 0.01; // Adjust sensitivity as needed
                        data.angle_y += delta.x * 0.01;
                        self.last_mouse_pos = mouse_event.pos;
                        ctx.request_paint();
                    } else if self.dragging_translation {
                        let delta = mouse_event.pos - self.last_mouse_pos;
                        // Update translation based on mouse movement
                        data.translation[0] += delta.x;
                        data.translation[1] += delta.y;
                        self.last_mouse_pos = mouse_event.pos;
                        ctx.request_paint();
                    }
                }
            }
            Event::MouseUp(mouse_event) => {
                if !data.paused {
                    match mouse_event.button {
                        druid::MouseButton::Left => {
                            self.dragging_rotation = false;
                        }
                        druid::MouseButton::Right => {
                            self.dragging_translation = false;
                        }
                        _ => {}
                    }
                    ctx.set_active(false);
                }
            }
            Event::Wheel(wheel_event) => {
                if !data.paused {
                    let delta = wheel_event.wheel_delta.y;
                    data.zoom *= 1.0 + delta * 0.001;
                    data.zoom = data.zoom.clamp(0.1, 10.0); // Clamp zoom level
                    ctx.request_paint();
                }
            }
            _ => {}
        }
    }

    fn lifecycle(
        &mut self,
        _ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        _data: &AppState,
        _env: &Env,
    ) {
        if let LifeCycle::Size(size) = event {
            self.size = *size;
        }
    }

    fn update(&mut self, _ctx: &mut UpdateCtx, _old_data: &AppState, _data: &AppState, _env: &Env) {
    }

    /// Determines the layout constraints for the cube widget
    fn layout(
        &mut self,
        _layout_ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        _data: &AppState,
        _env: &Env,
    ) -> Size {
        let size = bc.max();
        self.size = size;
        size
    }

    /// Paint the cube widget
    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppState, env: &Env) {
        // Update FPS calculation
        self.frames_since_last_update += 1;
        let now = Instant::now();
        let duration = now.duration_since(self.last_fps_calculation);
        if duration.as_secs_f64() >= 1.0 {
            self.fps = self.frames_since_last_update as f64 / duration.as_secs_f64();
            self.frames_since_last_update = 0;
            self.last_fps_calculation = now;
        }

        let size = ctx.size();
        let width = size.width as usize;
        let height = size.height as usize;

        // Create pixel buffer and z-buffer
        let mut pixel_data = vec![0u8; width * height * 4];
        let mut z_buffer = vec![std::f64::INFINITY; width * height];

        // Define face colors
        let face_colors = [
            Color::rgb8(255, 0, 0),   // Red
            Color::rgb8(0, 255, 0),   // Green
            Color::rgb8(0, 0, 255),   // Blue
            Color::rgb8(255, 255, 0), // Yellow
            Color::rgb8(255, 0, 255), // Magenta
            Color::rgb8(0, 255, 255), // Cyan
        ];

        // Light source position in world space
        let light_pos_world = data.light_position;

        let transformed_csg = self.csg
            .rotate(data.angle_x, data.angle_y, 0.0)
            .translate(data.translation[0], data.translation[1], 0.0)
            .scale(data.zoom, data.zoom, data.zoom);
        
        // Tessellate the transformed CSG into triangles.
        // (This will return a new CSG whose polygons are all triangles.)
        let tri_csg = transformed_csg.tessellate();
        
        // For each triangle, project its vertices and then draw it.
        for poly in tri_csg.polygons.iter() {
            if poly.vertices.len() == 3 {
                let p0 = Vertex {
                    position: poly.vertices[0].pos.coords.into(),
                    screen_position: self.project_point(&poly.vertices[0]),
                    normal: poly.vertices[0].normal.into(),
                };
                let p1 = Vertex {
                    position: poly.vertices[1].pos.coords.into(),
                    screen_position: self.project_point(&poly.vertices[1]),
                    normal: poly.vertices[1].normal.into(),
                };
                let p2 = Vertex {
                    position: poly.vertices[2].pos.coords.into(),
                    screen_position: self.project_point(&poly.vertices[2]),
                    normal: poly.vertices[2].normal.into(),
                };
                
                // Call your triangle drawing routine.
                // For example, if you have a function `draw_triangle`:
                draw_triangle(
                    &p0, &p1, &p2,
                    &mut pixel_data,
                    &mut z_buffer,
                    width,
                    height,
                    &light_pos_world,
                    face_colors[1],
                );
            }
        }

        // Create and draw the image
        let image = ctx
            .make_image(
                width,
                height,
                &pixel_data,
                druid::piet::ImageFormat::RgbaSeparate,
            )
            .unwrap();
        ctx.draw_image(&image, size.to_rect(), InterpolationMode::NearestNeighbor);

        // Add debug info if debug mode is enabled
        if data.debug {
            let text = format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            let text_layout = ctx
                .text()
                .new_text_layout(text)
                .font(FontFamily::SYSTEM_UI, 12.0)
                .text_color(Color::WHITE)
                .build()
                .unwrap();
            ctx.draw_text(&text_layout, (10.0, 10.0));

            // Draw angles
            let text = format!("Angle X: {:.2}, Angle Y: {:.2}", data.angle_x, data.angle_y);
            let text_layout = ctx
                .text()
                .new_text_layout(text)
                .font(FontFamily::SYSTEM_UI, 12.0)
                .text_color(Color::WHITE)
                .build()
                .unwrap();
            ctx.draw_text(&text_layout, (10.0, 30.0));

            // Draw translation
            let text = format!(
                "Translation X: {:.2}, Y: {:.2}",
                data.translation[0], data.translation[1]
            );
            let text_layout = ctx
                .text()
                .new_text_layout(text)
                .font(FontFamily::SYSTEM_UI, 12.0)
                .text_color(Color::WHITE)
                .build()
                .unwrap();
            ctx.draw_text(&text_layout, (10.0, 50.0));

            // Draw light position
            let text = format!(
                "Light: ({:.2}, {:.2}, {:.2})",
                light_pos_world[0], light_pos_world[1], light_pos_world[2]
            );
            let text_layout = ctx
                .text()
                .new_text_layout(text)
                .font(FontFamily::SYSTEM_UI, 12.0)
                .text_color(Color::WHITE)
                .build()
                .unwrap();
            ctx.draw_text(&text_layout, (10.0, 70.0));

            // Draw FPS
            let text = format!("FPS: {:.2}", self.fps);
            let text_layout = ctx
                .text()
                .new_text_layout(text)
                .font(FontFamily::SYSTEM_UI, 12.0)
                .text_color(Color::WHITE)
                .build()
                .unwrap();
            ctx.draw_text(&text_layout, (10.0, 90.0));

            // Draw zoom level
            let text = format!("Zoom: {:.2}", data.zoom);
            let text_layout = ctx
                .text()
                .new_text_layout(text)
                .font(FontFamily::SYSTEM_UI, 12.0)
                .text_color(Color::WHITE)
                .build()
                .unwrap();
            ctx.draw_text(&text_layout, (10.0, 110.0));
        }

        // Display 'Paused' if the simulation is paused
        if data.paused {
            // Draw a semi-transparent overlay
            let overlay_color = Color::rgba8(0, 0, 0, 150); // Adjust the alpha value as needed
            ctx.fill(size.to_rect(), &overlay_color);

            // Draw 'Paused' text
            let text = "Paused";
            let text_layout = ctx
                .text()
                .new_text_layout(text)
                .font(FontFamily::SYSTEM_UI, 36.0)
                .default_attribute(druid::piet::FontWeight::BOLD)
                .text_color(Color::WHITE)
                .build()
                .unwrap();
            let text_size = text_layout.size();
            let pos = (
                (size.width - text_size.width) / 2.0,
                (size.height - text_size.height) / 2.0,
            );
            ctx.draw_text(&text_layout, pos);
        }
    }
}
