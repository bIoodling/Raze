unsafe fn render_previews() {
    // Offscreen GDI renders, not interactive desktop screenshots.
    let out = std::env::args()
        .skip_while(|a| a != "--render-previews")
        .nth(1)
        .unwrap_or_else(|| "previews".into());
    fs::create_dir_all(&out).expect("preview directory");
    for dpi in [96, 120, 144, 192] {
        for (name, screen) in [
            ("compact", Screen::Compact),
            ("settings", Screen::Settings),
            ("confirm", Screen::Confirm),
            ("activation", Screen::Activate),
        ] {
            let (lw, lh) = screen_size(screen);
            let w = lw * dpi / 96;
            let h = lh * dpi / 96;
            {
                let mut ui = UI.get().unwrap().lock().unwrap();
                ui.screen = screen;
                ui.scale = dpi as f64 / 96.0;
            }
            let mut info: BITMAPINFO = zeroed();
            info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = w;
            info.bmiHeader.biHeight = -h;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let dc = CreateCompatibleDC(null_mut());
            let mut pixels: *mut c_void = null_mut();
            let bmp = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
            assert!(
                !bmp.is_null() && !pixels.is_null(),
                "offscreen render allocation failed"
            );
            let old = SelectObject(dc, bmp as HGDIOBJ);
            SetMapMode(dc, MM_ANISOTROPIC);
            SetWindowExtEx(dc, 96, 96, null_mut());
            SetViewportExtEx(dc, dpi, dpi, null_mut());
            draw_content(dc, screen, false, true);
            GdiFlush();
            let len = (w * h * 4) as usize;
            let mut bytes = Vec::with_capacity(54 + len);
            bytes.extend_from_slice(b"BM");
            bytes.extend_from_slice(&((54 + len) as u32).to_le_bytes());
            bytes.extend_from_slice(&[0u8; 4]);
            bytes.extend_from_slice(&54u32.to_le_bytes());
            bytes.extend_from_slice(&40u32.to_le_bytes());
            bytes.extend_from_slice(&w.to_le_bytes());
            bytes.extend_from_slice(&(-h).to_le_bytes());
            bytes.extend_from_slice(&1u16.to_le_bytes());
            bytes.extend_from_slice(&32u16.to_le_bytes());
            bytes.extend_from_slice(&[0u8; 24]);
            bytes.extend_from_slice(std::slice::from_raw_parts(pixels as *const u8, len));
            fs::write(
                PathBuf::from(&out).join(format!("{name}-{dpi}dpi.bmp")),
                bytes,
            )
            .expect("write render");
            SelectObject(dc, old);
            DeleteObject(bmp as HGDIOBJ);
            DeleteDC(dc);
        }
    }
}
