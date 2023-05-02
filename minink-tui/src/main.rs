use std::io;

use anyhow::Result;

use clap::Parser;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{select, FutureExt, StreamExt};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

use crate::app::App;

mod app;
mod ui;

#[derive(Debug, Parser)]
pub struct RunArgs {
    #[arg(short, long)]
    endpoints: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let run_args = RunArgs::parse();
    run(run_args).await?;
    Ok(())
}

pub async fn run(args: RunArgs) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(&args.endpoints);
    app.refresh().await?;
    run_app(&mut terminal, app).await?;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    let mut reader = EventStream::new();

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        let mut event = reader.next().fuse();

        select! {
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Char(c) => app.on_key(c).await?,
                                KeyCode::Left => app.on_left(),
                                KeyCode::Up => app.on_up(),
                                KeyCode::Right => app.on_right(),
                                KeyCode::Down => app.on_down(),
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(_)) => {},
                    Some(Err(e)) => println!("Error: {:?}\r", e),
                    None => break,
                }
            }
        };

        if app.should_quit {
            return Ok(());
        }
    }
    Ok(())
}
