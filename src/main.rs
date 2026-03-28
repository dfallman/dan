mod buffer;
mod config;
mod editor;
mod input;
mod render;
mod syntax;
mod utils;

use crossterm::event::{self, Event};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;

use std::env;
use std::io::{self, BufWriter};
use std::path::Path;
use std::time::Duration;

use crate::editor::Editor;

/// Version from the VERSION file (embedded at compile time).
pub const VERSION: &str = include_str!("../VERSION");

/// Short git hash (embedded at compile time by build.rs).
pub const GIT_HASH: &str = env!("GIT_HASH");

fn main() -> io::Result<()> {
	let args: Vec<String> = env::args().collect();

	// Handle version flags: -v, --v, --version
	if args.len() > 1 {
		let flag = args[1].as_str();
		if matches!(flag, "-v" | "--v" | "--version") {
			println!("dan {} ({})", VERSION.trim(), GIT_HASH);
			return Ok(());
		}
	}

	let mut editor = Editor::new();

	// Open file(s) from arguments
	if args.len() > 1 {
		let path = Path::new(&args[1]);
		if path.exists() {
			editor.open_file(path)?;
		} else {
			// Create a new buffer with the target path for saving
			editor.buffer_mut().file_path = Some(path.to_path_buf());
			editor.set_status(format!("[New File] {}", args[1]));
		}
	} else {
		editor.set_status("dan — a text editor | Ctrl+Q to quit");
	}

	// Set up terminal
	let stdout = io::stdout();
	let mut writer = BufWriter::with_capacity(64 * 1024, stdout);
	terminal::enable_raw_mode()?;
	writer.get_mut().execute(EnterAlternateScreen)?;

	// Ensure terminal state is gracefully recovered during fatal panics
	let default_panic_hook = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |panic_info| {
		let mut stdout = io::stdout();
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::event::DisableBracketedPaste);
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::style::ResetColor);
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::style::SetAttribute(crossterm::style::Attribute::Reset));
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::cursor::Show);
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::cursor::SetCursorStyle::DefaultUserShape);
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::terminal::LeaveAlternateScreen);
		let _ = crossterm::terminal::disable_raw_mode();
		default_panic_hook(panic_info);
	}));

	// Enable bracketed paste so the terminal sends paste as a
	// single Event::Paste(String) instead of individual key events.
	writer.get_mut().execute(crossterm::event::EnableBracketedPaste)?;

	// Main loop
	let result = run_loop(&mut editor, &mut writer);

	// Restore terminal
	writer.get_mut().execute(crossterm::event::DisableBracketedPaste)?;
	writer.get_mut().execute(crossterm::style::ResetColor)?;
	writer.get_mut().execute(crossterm::style::SetAttribute(crossterm::style::Attribute::Reset))?;
	writer.get_mut().execute(crossterm::cursor::Show)?;
	writer.get_mut().execute(crossterm::cursor::SetCursorStyle::DefaultUserShape)?;
	writer.get_mut().execute(LeaveAlternateScreen)?;
	terminal::disable_raw_mode()?;

	result
}

fn run_loop(editor: &mut Editor, writer: &mut BufWriter<io::Stdout>) -> io::Result<()> {
	loop {
		render::render(editor, writer)?;

		if editor.should_quit {
			break;
		}

		// Wait for an event, polling async tasks continuously.
		let evt = loop {
			let did_work = editor.poll_async_tasks();
			if did_work {
				render::render(editor, writer)?;
			}
			
			if event::poll(Duration::from_millis(25))? {
				break event::read()?;
			}
		};

		if matches!(evt, Event::Key(_) | Event::Paste(_))
			&& editor.mode != crate::editor::mode::Mode::Searching
			&& editor.mode != crate::editor::mode::Mode::ConfirmQuit
			&& editor.mode != crate::editor::mode::Mode::SaveAs
		{
			editor.clear_status();
		}

		// Handle resize events directly (not routed through Command).
		if let Event::Resize(w, h) = evt {
			editor.handle_resize(w, h);
		}

		let cmd = input::map_event(&evt, editor.mode);
		editor.execute(cmd);

		// Drain any additional buffered events without re-rendering.
		// This collapses rapid bursts of key events (e.g. fast typing
		// or unbatched paste in terminals that don't support bracketed
		// paste) into a single render pass.
		while event::poll(Duration::ZERO)? {
			let evt = event::read()?;
			if matches!(evt, Event::Key(_) | Event::Paste(_))
				&& editor.mode != crate::editor::mode::Mode::Searching
			&& editor.mode != crate::editor::mode::Mode::ConfirmQuit
			&& editor.mode != crate::editor::mode::Mode::SaveAs
			{
				editor.clear_status();
			}
			if let Event::Resize(w, h) = evt {
				editor.handle_resize(w, h);
			}
			let cmd = input::map_event(&evt, editor.mode);
			editor.execute(cmd);
		}
	}

	Ok(())
}
