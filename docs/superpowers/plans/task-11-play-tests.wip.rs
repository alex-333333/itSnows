// Task 11 tests, written and seen failing (World has no `play` yet).
// On resume: paste back at the end of `mod tests` in crates/snow-core/src/world.rs, then implement play.rs.

    // ---- play mode ----

    const GY: f32 = 1040.0;

    /// A fresh world with `h` px of snow on the ground. Fresh matters: no
    /// flake has had time to reach the ground, so snow totals are exact.
    fn play_world(h: f32) -> (World, [SurfaceIn; 1]) {
        let mut w = World::new(41);
        let surfaces = [ground(SW, GY)];
        step_s(&mut w, DT, HOLD, &surfaces);
        w.fill_ground(h);
        (w, surfaces)
    }

    fn ptr(w: &mut World, surfaces: &[SurfaceIn], x: f32, y: f32, left: bool, right: bool) -> StepOutput {
        let s = settings(HOLD);
        w.step(&FrameInput { dt: DT, width: SW, height: SH, surfaces, settings: &s, pointer: Some(Pointer { x, y, left, right }) })
    }

    /// Move the pointer from (x0,y0) to (x1,y1) at `speed` px per frame with the left button as given.
    fn glide(w: &mut World, surfaces: &[SurfaceIn], from: (f32, f32), to: (f32, f32), speed: f32, left: bool) {
        let dist = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2)).sqrt();
        let steps = (dist / speed).ceil().max(1.0) as usize;
        for k in 1..=steps {
            let u = k as f32 / steps as f32;
            ptr(w, surfaces, from.0 + (to.0 - from.0) * u, from.1 + (to.1 - from.1) * u, left, false);
        }
    }

    /// Roll a ball along the ground from x0 to x1 and let go.
    fn roll(w: &mut World, surfaces: &[SurfaceIn], x0: f32, x1: f32) {
        let y = GY - 5.0;
        ptr(w, surfaces, x0, y, false, false);
        ptr(w, surfaces, x0, y, true, false);
        glide(w, surfaces, (x0, y), (x1, y), 4.0, true);
        ptr(w, surfaces, x1, y, false, false);
    }

    /// Pick up the free ball at `from_x` and set it down gently at `to_x`.
    fn carry(w: &mut World, surfaces: &[SurfaceIn], from_x: f32, to_x: f32) {
        let grab = (from_x, GY - 8.0);
        let up = (from_x, GY - 300.0);
        let over = (to_x, GY - 300.0);
        ptr(w, surfaces, grab.0, grab.1, false, false);
        ptr(w, surfaces, grab.0, grab.1, true, false);
        glide(w, surfaces, grab, up, 8.0, true);
        glide(w, surfaces, up, over, 8.0, true);
        for _ in 0..15 {
            ptr(w, surfaces, over.0, over.1, true, false);
        }
        ptr(w, surfaces, over.0, over.1, false, false);
    }

    fn ball_radii(w: &World) -> Vec<f32> {
        w.play.stacks.iter().flat_map(|s| s.balls.iter().map(|b| b.r)).collect()
    }

    fn total_snow(w: &World) -> f32 {
        w.ground().unwrap().total_area() + w.play.total_area()
    }

    #[test]
    fn rolling_conserves_snow() {
        let (mut w, surfaces) = play_world(20.0);
        let before = total_snow(&w);
        roll(&mut w, &surfaces, 300.0, 700.0);
        assert_eq!(w.ball_count(), 1);
        assert!((total_snow(&w) - before).abs() < 1.0, "{} vs {before}", total_snow(&w));
        assert!(w.play.total_area() > 7000.0);
    }

    #[test]
    fn ball_grows_with_distance() {
        let (mut w, surfaces) = play_world(20.0);
        roll(&mut w, &surfaces, 300.0, 360.0);
        roll(&mut w, &surfaces, 600.0, 800.0);
        let r = ball_radii(&w);
        assert_eq!(r.len(), 2);
        assert!(r[1] > r[0] * 1.5, "{r:?}");
    }

    #[test]
    fn rolling_leaves_a_track() {
        let (mut w, surfaces) = play_world(20.0);
        let y = GY - 5.0;
        ptr(&mut w, &surfaces, 300.0, y, true, false);
        glide(&mut w, &surfaces, (300.0, y), (600.0, y), 4.0, true);
        assert!(w.ground().unwrap().height_at(450.0) < 20.0);
        assert_eq!(w.ground().unwrap().height_at(900.0), 20.0);
    }

    #[test]
    fn tiny_ball_returns_snow() {
        let (mut w, surfaces) = play_world(1.0);
        let before = total_snow(&w);
        roll(&mut w, &surfaces, 300.0, 304.0);
        assert_eq!(w.ball_count(), 0);
        assert!((w.ground().unwrap().total_area() - before).abs() < 0.5);
    }

    #[test]
    fn radius_capped() {
        let (mut w, surfaces) = play_world(80.0);
        roll(&mut w, &surfaces, 100.0, 1800.0);
        let r = ball_radii(&w);
        assert!((r[0] - BALL_MAX_R).abs() < 0.01, "{r:?}");
        // Past the cap it stops eating snow.
        assert_eq!(w.ground().unwrap().height_at(1700.0), 80.0);
    }

    #[test]
    fn balls_rest_on_the_ground() {
        let (mut w, surfaces) = play_world(20.0);
        roll(&mut w, &surfaces, 300.0, 440.0);
        let b = &w.play.stacks[0].balls[0];
        assert!((b.y + b.r - GY).abs() < 0.01);
        assert!((b.x - 440.0).abs() < 0.01);
    }

    #[test]
    fn smaller_stacks_on_larger() {
        let (mut w, surfaces) = play_world(20.0);
        roll(&mut w, &surfaces, 300.0, 440.0);
        roll(&mut w, &surfaces, 600.0, 664.0);
        carry(&mut w, &surfaces, 664.0, 445.0);
        assert_eq!(w.play.stacks.len(), 1);
        let balls = &w.play.stacks[0].balls;
        assert_eq!(balls.len(), 2);
        assert!(balls[1].r < balls[0].r);
        assert_eq!(balls[1].x, balls[0].x);
        assert!(balls[1].y < balls[0].y - balls[0].r * 0.5);
        assert_eq!(w.ball_count(), 2);
    }

    #[test]
    fn larger_does_not_stack() {
        let (mut w, surfaces) = play_world(20.0);
        roll(&mut w, &surfaces, 300.0, 440.0);
        roll(&mut w, &surfaces, 600.0, 664.0);
        carry(&mut w, &surfaces, 440.0, 664.0);
        assert_eq!(w.play.stacks.len(), 2);
        let (a, b) = (&w.play.stacks[0].balls[0], &w.play.stacks[1].balls[0]);
        assert!((a.x - b.x).abs() >= a.r + b.r - 0.01, "balls overlap");
        assert!((a.y + a.r - GY).abs() < 0.01 && (b.y + b.r - GY).abs() < 0.01);
    }

    fn build_snowman(w: &mut World, surfaces: &[SurfaceIn]) {
        roll(w, surfaces, 300.0, 440.0);
        roll(w, surfaces, 600.0, 664.0);
        roll(w, surfaces, 800.0, 824.0);
        carry(w, surfaces, 664.0, 440.0);
        carry(w, surfaces, 824.0, 440.0);
    }

    #[test]
    fn three_make_a_snowman() {
        let (mut w, surfaces) = play_world(20.0);
        build_snowman(&mut w, &surfaces);
        assert_eq!(w.snowman_count(), 1);
        assert_eq!(w.ball_count(), 0);
    }

    #[test]
    fn snowman_is_drawn_with_a_face() {
        let (mut w, surfaces) = play_world(20.0);
        build_snowman(&mut w, &surfaces);
        step_s(&mut w, DT, HOLD, &surfaces);
        let mut out = Vec::new();
        w.draw(&mut out);
        let orange = out.iter().any(|p| matches!(p, Prim::Polygon { color, .. } if color.r > 0.9 && color.g > 0.3 && color.g < 0.7 && color.b < 0.3));
        assert!(orange, "carrot nose");
        let dark = out.iter().filter(|p| matches!(p, Prim::Circle { color, .. } if color.r < 0.2 && color.g < 0.2)).count();
        assert!(dark >= 4, "eyes and buttons");
    }

    #[test]
    fn fourth_ball_does_not_stack() {
        let (mut w, surfaces) = play_world(20.0);
        build_snowman(&mut w, &surfaces);
        roll(&mut w, &surfaces, 1000.0, 1012.0);
        carry(&mut w, &surfaces, 1012.0, 440.0);
        assert_eq!(w.snowman_count(), 1);
        assert_eq!(w.ball_count(), 1);
        assert_eq!(w.play.stacks.iter().map(|s| s.balls.len()).max(), Some(3));
    }

    #[test]
    fn snowman_cannot_be_picked_apart() {
        let (mut w, surfaces) = play_world(20.0);
        build_snowman(&mut w, &surfaces);
        let head = w.play.stacks[0].balls[2].clone();
        ptr(&mut w, &surfaces, head.x, head.y, true, false);
        glide(&mut w, &surfaces, (head.x, head.y), (head.x + 300.0, head.y), 8.0, true);
        ptr(&mut w, &surfaces, head.x + 300.0, head.y, false, false);
        assert_eq!(w.snowman_count(), 1);
        assert_eq!(w.play.stacks[0].balls.len(), 3);
    }

    #[test]
    fn fast_release_throws_and_bursts() {
        let (mut w, surfaces) = play_world(20.0);
        roll(&mut w, &surfaces, 300.0, 440.0);
        let grab = (440.0, GY - 8.0);
        ptr(&mut w, &surfaces, grab.0, grab.1, true, false);
        // 15 px per frame = 900 px/s, up and to the right.
        glide(&mut w, &surfaces, grab, (740.0, GY - 308.0), 15.0, true);
        let flakes_before = w.flake_count();
        ptr(&mut w, &surfaces, 740.0, GY - 308.0, false, false);
        assert_eq!(w.play.flying.len(), 1);
        assert_eq!(w.play.stacks.len(), 0);
        let mut t = 0.0;
        while w.ball_count() > 0 && t < 5.0 {
            ptr(&mut w, &surfaces, 740.0, GY - 308.0, false, false);
            t += DT;
        }
        assert_eq!(w.ball_count(), 0, "ball should have burst");
        let burst = w.flake_count() as i64 - flakes_before as i64;
        assert!(burst > 50 && burst <= 220, "{burst}");
    }

    #[test]
    fn thrown_ball_bursts_on_a_window_top() {
        let mut w = World::new(41);
        let surfaces = [ground(SW, GY), window(5, 600.0, 1400.0, 700.0)];
        step_s(&mut w, DT, HOLD, &surfaces);
        w.fill_ground(20.0);
        roll(&mut w, &surfaces, 300.0, 440.0);
        let grab = (440.0, GY - 8.0);
        ptr(&mut w, &surfaces, grab.0, grab.1, true, false);
        glide(&mut w, &surfaces, grab, (640.0, GY - 608.0), 15.0, true);
        ptr(&mut w, &surfaces, 640.0, GY - 608.0, false, false);
        let mut burst_y = None;
        for _ in 0..600 {
            let flying_y = w.play.flying.first().map(|f| f.ball.y);
            ptr(&mut w, &surfaces, 640.0, GY - 608.0, false, false);
            if w.play.flying.is_empty() {
                burst_y = flying_y;
                break;
            }
        }
        let y = burst_y.expect("ball should have burst");
        assert!(y < 720.0, "burst at y={y}, expected on the window top at 700");
    }

    #[test]
    fn right_click_crumbles_snowman() {
        let (mut w, surfaces) = play_world(20.0);
        build_snowman(&mut w, &surfaces);
        let before = total_snow(&w);
        let ground_before = w.ground().unwrap().total_area();
        let base = w.play.stacks[0].balls[0].clone();
        ptr(&mut w, &surfaces, base.x, base.y, false, false);
        ptr(&mut w, &surfaces, base.x, base.y, false, true);
        assert_eq!(w.snowman_count(), 0);
        assert_eq!(w.play.stacks.len(), 0);
        assert!(w.ground().unwrap().total_area() > ground_before + 1000.0);
        assert!(total_snow(&w) <= before + 1.0);
    }

    #[test]
    fn timeout_requests_exit() {
        let (mut w, surfaces) = play_world(20.0);
        let mut exited = false;
        for _ in 0..(29.0 / DT) as usize {
            exited |= ptr(&mut w, &surfaces, 500.0, 500.0, false, false).exit_play;
        }
        assert!(!exited, "too early");
        for _ in 0..(1.2 / DT) as usize {
            exited |= ptr(&mut w, &surfaces, 500.0, 500.0, false, false).exit_play;
        }
        assert!(exited);
    }

    #[test]
    fn movement_resets_timeout() {
        let (mut w, surfaces) = play_world(20.0);
        let mut exited = false;
        for i in 0..(40.0 / DT) as usize {
            let x = 500.0 + (i / 600 % 2) as f32 * 30.0;
            exited |= ptr(&mut w, &surfaces, x, 500.0, false, false).exit_play;
        }
        assert!(!exited);
    }

    #[test]
    fn leaving_play_mode_resets_timeout_and_drops_held_ball() {
        let (mut w, surfaces) = play_world(20.0);
        for _ in 0..(20.0 / DT) as usize {
            ptr(&mut w, &surfaces, 500.0, 500.0, false, false);
        }
        let y = GY - 5.0;
        ptr(&mut w, &surfaces, 300.0, y, true, false);
        glide(&mut w, &surfaces, (300.0, y), (440.0, y), 4.0, true);
        step_s(&mut w, DT, HOLD, &surfaces);
        assert_eq!(w.play.stacks.len(), 1, "held ball is set down");
        let mut exited = false;
        for _ in 0..(20.0 / DT) as usize {
            exited |= ptr(&mut w, &surfaces, 500.0, 500.0, false, false).exit_play;
        }
        assert!(!exited);
    }

    #[test]
    fn no_play_without_pointer() {
        let (mut w, surfaces) = play_world(20.0);
        let out = step_s(&mut w, DT, HOLD, &surfaces);
        assert!(!out.exit_play);
        let mut prims = Vec::new();
        w.draw(&mut prims);
        assert!(!prims.iter().any(|p| matches!(p, Prim::RoundRect { .. })));
    }

    #[test]
    fn play_mode_draws_a_border_last() {
        let (mut w, surfaces) = play_world(20.0);
        let out = ptr(&mut w, &surfaces, 500.0, 500.0, false, false);
        assert!(!out.idle);
        let mut prims = Vec::new();
        w.draw(&mut prims);
        assert!(prims.len() >= 4);
        assert!(prims[prims.len() - 4..].iter().all(|p| matches!(p, Prim::RoundRect { .. })));
    }

    #[test]
    fn press_with_no_ground_is_safe() {
        let mut w = World::new(42);
        ptr(&mut w, &[], 300.0, 1000.0, true, false);
        glide(&mut w, &[], (300.0, 1000.0), (700.0, 1000.0), 4.0, true);
        ptr(&mut w, &[], 700.0, 1000.0, false, true);
        assert_eq!(w.ball_count(), 0);
    }

    #[test]
    fn press_far_above_the_snow_does_nothing() {
        let (mut w, surfaces) = play_world(20.0);
        ptr(&mut w, &surfaces, 300.0, 500.0, true, false);
        glide(&mut w, &surfaces, (300.0, 500.0), (700.0, 500.0), 4.0, true);
        ptr(&mut w, &surfaces, 700.0, 500.0, false, false);
        assert_eq!(w.ball_count(), 0);
        assert_eq!(w.ground().unwrap().height_at(500.0), 20.0);
    }

    #[test]
    fn balls_survive_clear_snow_and_blower() {
        let (mut w, surfaces) = play_world(20.0);
        build_snowman(&mut w, &surfaces);
        w.clear_snow();
        w.call_blower();
        step_s(&mut w, DT, HOLD, &surfaces);
        run_until_blower_done(&mut w, &surfaces, 60.0);
        assert_eq!(w.snowman_count(), 1);
    }

    #[test]
    fn balls_follow_the_ground_line() {
        let (mut w, _) = play_world(20.0);
        roll(&mut w, &[ground(SW, GY)], 300.0, 440.0);
        step_s(&mut w, DT, HOLD, &[ground(SW, GY - 100.0)]);
        let b = &w.play.stacks[0].balls[0];
        assert!((b.y + b.r - (GY - 100.0)).abs() < 0.01);
    }
