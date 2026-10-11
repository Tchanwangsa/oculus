//! Keep the initial Windows window inside its monitor's usable desktop.

/// All values are physical pixels: DPI has already been applied by the OS.
fn fit_axis(position: i32, inner: u32, outer: u32, origin: i32, available: u32) -> (i32, u32) {
    let frame = outer.saturating_sub(inner);
    let size = inner.min(available.saturating_sub(frame).max(1));
    let last = (i64::from(origin) + i64::from(available) - i64::from(size) - i64::from(frame))
        .max(i64::from(origin));
    let position = i64::from(position).clamp(i64::from(origin), last);
    (position as i32, size)
}

#[cfg(windows)]
pub(crate) fn fit_startup(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::Manager;
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let monitor = match window.current_monitor()? {
        Some(monitor) => monitor,
        None => match window.primary_monitor()? {
            Some(monitor) => monitor,
            None => return Ok(()),
        },
    };
    let area = monitor.work_area();
    let inner = window.inner_size()?;
    let outer = window.outer_size()?;
    let position = window.outer_position()?;
    let (x, width) = fit_axis(
        position.x,
        inner.width,
        outer.width,
        area.position.x,
        area.size.width,
    );
    let (y, height) = fit_axis(
        position.y,
        inner.height,
        outer.height,
        area.position.y,
        area.size.height,
    );

    // A display can have less logical space than the configured minimum at
    // high DPI. Allow this initial fit instead of forcing part of it offscreen.
    if let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|config| config.label == "main")
    {
        let scale = window.scale_factor()?;
        let available_width = area
            .size
            .width
            .saturating_sub(outer.width.saturating_sub(inner.width));
        let available_height = area
            .size
            .height
            .saturating_sub(outer.height.saturating_sub(inner.height));
        let min_width = config.min_width.unwrap_or(0.0);
        let min_height = config.min_height.unwrap_or(0.0);
        if min_width * scale > f64::from(available_width)
            || min_height * scale > f64::from(available_height)
        {
            window.set_min_size(Some(tauri::LogicalSize::new(
                min_width.min(f64::from(available_width) / scale),
                min_height.min(f64::from(available_height) / scale),
            )))?;
        }
    }
    if width != inner.width || height != inner.height {
        window.set_size(tauri::PhysicalSize::new(width, height))?;
    }
    if x != position.x || y != position.y {
        window.set_position(tauri::PhysicalPosition::new(x, y))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fit_axis;

    #[test]
    fn high_dpi_startup_fits_the_work_area_and_keeps_the_taskbar_clear() {
        // 1480 x 920 logical pixels at 150%, on the VM's 1365 x 960 desktop.
        assert_eq!(fit_axis(0, 2220, 2220, 0, 1365), (0, 1365));
        assert_eq!(fit_axis(0, 1380, 1380, 0, 900), (0, 900));
        assert_eq!(fit_axis(0, 2220, 2236, 0, 1365), (0, 1349));
    }

    #[test]
    fn a_window_that_fits_keeps_its_size_and_position() {
        assert_eq!(fit_axis(200, 900, 916, 0, 1920), (200, 900));
        assert_eq!(fit_axis(1800, 900, 916, 0, 1920), (1004, 900));
    }

    #[test]
    fn a_monitor_left_of_the_primary_uses_its_negative_origin() {
        assert_eq!(fit_axis(-100, 900, 916, -1920, 1920), (-916, 900));
        assert_eq!(fit_axis(-2200, 900, 916, -1920, 1920), (-1920, 900));
    }
}
