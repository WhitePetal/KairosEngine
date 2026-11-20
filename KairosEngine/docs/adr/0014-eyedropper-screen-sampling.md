# ADR 0014: The eyedropper samples the desktop through a host seam

- **Status**: Accepted
- **Date**: 2026-11-20
- **Context**:
  - The color picker has had a Unity-style eyedropper affordance since it was
    written, but no sampling: the button only showed a tooltip. Unity's
    eyedropper samples "any pixel, in or outside of the Unity Editor"
    ([Unity Manual](https://docs.unity3d.com/6000.3/Documentation/Manual/InspectorColorPicker.html)),
    and the editor's picker is modelled on Unity's.
  - `kairos_editor_widgets` is pure egui by contract: it may not depend on the
    engine, wgpu, or a platform API. Pixels and the OS pointer therefore belong
    to the host, not to the widget.
  - Two mechanisms were considered. **(a) Frame readback**: copy the editor's
    own swapchain and read the pixel under the cursor. wgpu only guarantees
    `RENDER_ATTACHMENT` on surfaces; egui-wgpu's own screenshot support renders
    to an offscreen texture first for exactly that reason, and the engine draws
    3D and egui into one pass on the surface, so a capture target would have to
    be added to the frame model. It also cannot see outside the window.
    **(b) OS sampling**: read the desktop under the OS cursor. It matches Unity,
    needs no render-graph work, and is the only way to pick from another window.
  - winit/egui only report the pointer while it is over the window, and a pick
    must continue over other windows; egui also cannot draw outside its window,
    so the near-cursor feedback needs an OS window.
  - Screen coordinates are physical pixels; egui is logical points. A desktop
    sample is display-referred sRGB, and an LDR screen cannot recover values
    above 1.0.
- **Decision**:
  1. **The widget owns the pick, the host owns the pointer.**
     `color_picker::eyedropper` exposes the session (`session`), a capability
     flag (`set_supported`), and one `Update { id, field, srgb, outcome }` per
     frame (`publish`). The widget applies samples, restores on `Cancelled`, and
     ends the session on `Committed`. It never knows the platform; a host that
     cannot sample publishes nothing and the affordance keeps its tooltip.
  2. **The host samples the desktop.** `kairos_editor_app/src/eyedropper/win32.rs`
     polls `GetCursorPos`, `BitBlt`s a 15x15 block from the screen DC, draws a
     topmost click-through GDI loupe next to the pointer (magnified pixels,
     centre cell, `#hex`, RGB and a colour chip), and reads the confirm/cancel
     click from `GetAsyncKeyState` so a pick can end anywhere on the desktop.
     While a pick runs a `WH_MOUSE_LL` hook discards the desktop's button and
     wheel events, so the confirming click cannot press a widget, another
     application, or this window's own caption buttons — mouse capture alone
     does not cover the capture window's non-client area. Hover moves must keep
     flowing: the system moves the cursor while it delivers them, so a
     discarded move freezes the pointer (measured — the first mask did exactly
     that). A discarded button event is invisible to `GetAsyncKeyState` too, so
     the hook records the confirming/cancelling press itself and the pick reads
     its clicks from that record (a failed hook falls back to capture plus the
     key state). Keyboard input stays live so Esc can cancel (and so the pick
     cannot trap the user).
     `unsupported.rs` reports `available_for(window) == false` on platforms
     without a backend.
  3. **A pick changes only the base colour.** The sample is re-composed at the
     session's exposure (`ColorProperty::with_base`, alpha preserved), and the
     picker's Intensity row is refreshed base-only while a pick runs, so an HDR
     pick never drifts the brightness it started with.
  4. **The engine's frame is not the eyedropper's source.** A wgpu capture
     remains the right mechanism for screenshots and viewport thumbnails, not
     for this feature.
- **Consequences**:
  - **The platform backend stays in the app crate.** `kairos_editor_widgets` may
    only depend on egui and `kairos_color` ([ADR 0008](./0008-editor-ui-architecture.md),
    `widget-layer.md`), so Win32 sampling could not live there without breaking
    the boundary the Cargo manifest enforces. The widget crate owns the
    *feature* — session, affordance, live application, commit/cancel semantics,
    HDR recomposition — and the app crate owns the *OS adapter*: pixels, the
    loupe window, the input mask. That is the same shape as `window_controls`
    (the widget draws and decides, the host performs the window action), and a
    second platform only adds a host backend. The widget half is also testable
    without any platform: the tests publish `Update`s straight into the context.
  - The mask makes the pick modal at the desktop level: while it runs, no
    window reacts to the pointer's buttons, including this window's caption
    buttons, the taskbar and other applications; moves still flow so the
    cursor keeps moving, and with the capture held they reach the editor and
    are kept away from egui. A click still confirms the pick because the hook
    records the masked press; polling `GetAsyncKeyState` is not enough — an
    event the hook discards never reaches it (measured: with the first mask a
    click on Close kept the window alive but did not end the pick). The hook
    has to answer quickly (it only compares a message id); Windows removes a
    low-level hook whose procedure stops answering, which is the fallback if
    the event loop stalls. Alt+F4 and the taskbar thumbnail's Close button are
    not mouse messages on the desktop and stay live.
  - The click that confirms a pick never reaches egui, so the widget reports the
    pick's end of gesture (`eyedropper::take_committed`) and Preferences commits
    the page on it; otherwise the colour would show up live but the `.ron` write
    would wait for the next ordinary pointer release. The host also keeps
    consuming mouse events while a pick runs and until the ending click is
    released — the fallback path when the hook cannot be installed — and no
    synthetic release is injected, because egui's pointer never moved during the
    pick and a release at that stale position lands on the eyedropper button and
    starts a new pick — measured, not hypothetical.
  - The loupe is the near-cursor feedback; the egui field and picker also show
    the live value when they are visible, one frame behind the sample.
  - A pick can outlive its field (the panel closes mid-pick). The session times
    out after 60 s and the host stops the sampler. If the host already published
    the pick's terminal update and no field consumed it, it does not start a
    second sampler for that session: it waits for a field to draw the update or
    for the session timeout.
  - HDR eyedropping stays LDR: the sample is the displayed sRGB byte. Sampling
    scene-linear HDR values needs a float render target and is out of scope.
  - Wayland cannot sample outside the window by protocol; its ScreenCast
    portal remains the only route and is not implemented. macOS uses
    ScreenCaptureKit when it is available, falling back to the permission-free
    system sampler (see the update below); X11 uses a private X connection.

- **Update (2026-11-20): macOS and X11 backends**:
  - **macOS samples with ScreenCaptureKit, with the system sampler as a
    fallback.** The backend asks for Screen Recording directly with
    `CGRequestScreenCaptureAccess` on first use, then requests the displays
    from `SCShareableContent`. While
    the pick runs, one transparent full-screen window per screen carries a 15x15
    loupe, and the view under the pointer draws the pixel grid, `#RRGGBB`, RGB
    and a swatch. The live sRGB sample is published every frame, so the editor
    field follows the pointer just as it does on Windows/X11. A capture asks for
    exactly the 15 screen pixels around the pointer: `sourceRect` is in
    display-local points, sized `15 / pointPixelScale` and snapped to the
    device-pixel grid, with `scalesToFit = false`; the image is drawn into an
    sRGB bitmap context for the byte order and P3 conversion. Left click
    commits; right click and Esc cancel. The overlay consumes the confirming
    click, so the host's release guard applies on this path too. The overlay's
    flip from AppKit's bottom-left frames to the top-left points used by the
    capture and the loupe is anchored to the menu-bar screen (the screen whose
    frame origin is `(0, 0)`, always `NSScreen.screens[0]`), never to
    `NSScreen.mainScreen`: the latter is the screen with the keyboard focus,
    so anchoring to it would shift every point as soon as the editor window
    moves to another display and a pick there would sample the wrong screen at
    the wrong place. If
    `SCScreenshotManager` is unavailable (macOS before 14), the content request
    fails, or the user denies Screen Recording, the pick falls back to
    `NSColorSampler` — AppKit's permission-free system eyedropper. That fallback
    is usable but reports only at the end and has no right-click cancel. The
    ScreenCaptureKit framework link makes macOS 12.3 the effective runtime
    floor; on 12.3–13 the fallback is the only path.
  - **Linux samples X11 directly.** The backend opens its own X connection,
    reads the pointer with `QueryPointer`, copies a clipped 15x15 `GetImage`
    block around it, and draws the loupe in an override-redirect window (a
    comparable layout, painted into an X image instead of a GDI DIB). A
    `GrabPointer` with only button events selected discards the desktop's
    buttons for the pick while the cursor keeps moving (motion is not selected
    and is not needed); the confirming press is read from the grab's event
    queue and from the button state, and Esc is polled with `QueryKeymap` so
    the keyboard stays live. Wayland is
    unavailable: there is no global pointer and no cross-window pixels without
    the ScreenCast portal. XWayland is treated the same way, since its root
    window does not contain Wayland clients' pixels.
  - **The release guard is now frame-based.** Windows and X11 hide the
    confirming press from every window, so the host must also swallow the
    release that follows: egui's pointer never moved during the pick, and a
    release at that stale position lands on the eyedropper button (the earlier
    implementation measured it starting a new pick). The host stops swallowing
    only after two consecutive frames observe no mouse button down (the X11
    backend keeps its connection just long enough to answer that question).
    macOS's ScreenCaptureKit overlay consumes the confirming click, so it arms
    the guard like Windows/X11; the `NSColorSampler` fallback owns the whole
    click and does not.
