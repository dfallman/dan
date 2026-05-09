// SPDX-License-Identifier: GPL-3.0-or-later

/*
 * Dan -- a fast, friendly, and zero-fuss terminal text editor.
 * Copyright (C) 2026 Daniel Fallman
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/
	
mod buffer;
mod config;
mod atomic_io;
mod editor;
mod input;
pub mod recovery;
mod render;
mod sanitize;
mod syntax;
pub mod ui;
mod utils;

use crossterm::event::{self, Event};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;

use std::env;
use std::io::{self, BufWriter};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::editor::Editor;

/// Version from the VERSION file (embedded at compile time).
pub const VERSION: &str = include_str!("../VERSION");

/// Short git hash (embedded at compile time by build.rs).
pub const GIT_HASH: &str = env!("GIT_HASH");

/// Register handlers for SIGTERM, SIGHUP, SIGQUIT that flip the returned
/// flag. The main loop polls this flag and triggers a graceful shutdown,
/// ensuring the terminal is restored from raw mode + alt screen instead of
/// being stranded after `kill` / SSH drop / parent-process exit. SIGINT is
/// not registered here — crossterm's raw mode delivers Ctrl-C as a normal
/// `KeyEvent` instead. On non-Unix the returned flag is never flipped.
fn install_signal_shutdown_flag() -> io::Result<Arc<AtomicBool>> {
	let flag = Arc::new(AtomicBool::new(false));
	#[cfg(unix)]
	{
		use signal_hook::consts::{SIGHUP, SIGQUIT, SIGTERM};
		signal_hook::flag::register(SIGTERM, Arc::clone(&flag))?;
		signal_hook::flag::register(SIGHUP, Arc::clone(&flag))?;
		signal_hook::flag::register(SIGQUIT, Arc::clone(&flag))?;
	}
	Ok(flag)
}

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
			if let Err(e) = editor.open_file(path) {
				eprintln!("dan: Could not open '{}': {}", path.display(), e);
				std::process::exit(1);
			}
		} else {
			// Create a new buffer with the target path for saving
			editor.buffer_mut().file_path = Some(path.to_path_buf());
			editor.config.apply_editorconfig(path); // Apply layout rules perfectly even for new files!
			editor.set_status(format!("[New File] {}", args[1]));
		}
	} else {
		editor.set_status("dan's text editor | ^Q to quit");
	}

	// Install signal handlers BEFORE entering raw mode so a signal arriving
	// during the next few statements still trips the cleanup path.
	let shutdown_signal = install_signal_shutdown_flag()?;

	// Set up terminal
	let stdout = io::stdout();
	let mut writer = BufWriter::with_capacity(64 * 1024, stdout);
	terminal::enable_raw_mode()?;
	writer.get_mut().execute(EnterAlternateScreen)?;

	// Ensure terminal state is gracefully recovered during fatal panics
	let default_panic_hook = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |panic_info| {
		let mut stdout = io::stdout();
		let _ = crossterm::ExecutableCommand::execute(
			&mut stdout,
			crossterm::event::DisableBracketedPaste,
		);
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::style::ResetColor);
		let _ = crossterm::ExecutableCommand::execute(
			&mut stdout,
			crossterm::style::SetAttribute(crossterm::style::Attribute::Reset),
		);
		let _ = crossterm::ExecutableCommand::execute(&mut stdout, crossterm::cursor::Show);
		let _ = crossterm::ExecutableCommand::execute(
			&mut stdout,
			crossterm::cursor::SetCursorStyle::DefaultUserShape,
		);
		let _ = crossterm::ExecutableCommand::execute(
			&mut stdout,
			crossterm::terminal::LeaveAlternateScreen,
		);
		let _ = crossterm::terminal::disable_raw_mode();
		default_panic_hook(panic_info);
	}));

	// Enable bracketed paste so the terminal sends paste as a
	// single Event::Paste(String) instead of individual key events.
	writer
		.get_mut()
		.execute(crossterm::event::EnableBracketedPaste)?;

	// Main loop
	let result = run_loop(&mut editor, &mut writer, &shutdown_signal);

	// Restore terminal
	writer
		.get_mut()
		.execute(crossterm::event::DisableBracketedPaste)?;
	writer.get_mut().execute(crossterm::style::ResetColor)?;
	writer.get_mut().execute(crossterm::style::SetAttribute(
		crossterm::style::Attribute::Reset,
	))?;
	writer.get_mut().execute(crossterm::cursor::Show)?;
	writer
		.get_mut()
		.execute(crossterm::cursor::SetCursorStyle::DefaultUserShape)?;
	writer.get_mut().execute(LeaveAlternateScreen)?;
	terminal::disable_raw_mode()?;

	result
}

fn run_loop(
	editor: &mut Editor,
	writer: &mut BufWriter<io::Stdout>,
	shutdown_signal: &Arc<AtomicBool>,
) -> io::Result<()> {
	loop {
		// A SIGTERM/SIGHUP/SIGQUIT triggers graceful shutdown via the
		// existing terminal-restoration path in `main`. The most-recent
		// state should already be in the autosave swap-file (5s cadence).
		if shutdown_signal.load(Ordering::Relaxed) {
			editor.should_quit = true;
		}

		render::render(editor, writer)?;

		if editor.should_quit {
			break;
		}

		// Wait for an event, polling async tasks continuously.
		let evt = loop {
			if shutdown_signal.load(Ordering::Relaxed) {
				editor.should_quit = true;
				return Ok(());
			}
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

		// Drain extra buffered events with a 5ms micro-timeout so a burst
		// of key events (fast typing, continuous scrolling) collapses into
		// one render pass.
		while event::poll(Duration::from_millis(5))? {
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
