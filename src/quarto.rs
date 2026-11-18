use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use egui::{
    Align, Button, Color32, FontId, Frame, Margin, RichText, Rounding, Sense, Stroke, Ui, Vec2,
};
use crate::export::{clean_filename, TyporaStylesheet};
use crate::math::MathRenderer;
use crate::theme::ColorPalette;

#[derive(Clone, Debug, PartialEq)]
pub struct QuartoMetadata {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub format: String,
    pub raw_yaml: String,
}

impl Default for QuartoMetadata {
    fn default() -> Self {
        Self {
            title: None,
            subtitle: None,
            author: None,
            date: None,
            format: "html".to_string(),
            raw_yaml: String::new(),
        }
    }
}

pub fn parse_quarto_frontmatter(text: &str) -> Option<(QuartoMetadata, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let mut start_idx = 0;
    while start_idx < lines.len() && lines[start_idx].trim().is_empty() {
        start_idx += 1;
    }
    if start_idx >= lines.len() || lines[start_idx].trim() != "---" {
        return None;
    }

    let mut end_idx = None;
    for i in (start_idx + 1)..lines.len() {
        let t = lines[i].trim();
        if t == "---" || t == "..." {
            end_idx = Some(i);
            break;
        }
    }

    let end_idx = end_idx?;
    let yaml_lines = &lines[(start_idx + 1)..end_idx];
    let raw_yaml = yaml_lines.join("\n");

    // A leading `---` is only frontmatter when it actually contains keys;
    // otherwise it is a horizontal rule and must render as one.
    let has_key = yaml_lines.iter().any(|l| {
        let trimmed = l.trim();
        !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && trimmed
                .split_once(':')
                .is_some_and(|(k, _)| !k.trim().is_empty() && !k.trim().contains(' '))
    });
    if !has_key {
        return None;
    }

    let mut meta = QuartoMetadata {
        raw_yaml,
        ..Default::default()
    };

    for line in yaml_lines {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        if let Some((key, val)) = trimmed.split_once(':') {
            let key = key.trim().to_lowercase();
            let val = val.trim().trim_matches('"').trim_matches('\'').trim().to_string();
            match key.as_str() {
                "title" => meta.title = Some(val),
                "subtitle" => meta.subtitle = Some(val),
                "author" => meta.author = Some(val),
                "date" => meta.date = Some(val),
                "format" => {
                    if !val.is_empty() {
                        meta.format = val;
                    }
                }
                _ => {}
            }
        }
    }

    Some((meta, end_idx + 1))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuartoCalloutType {
    Note,
    Tip,
    Warning,
    Important,
    Caution,
}

impl QuartoCalloutType {
    pub fn from_str(s: &str) -> Option<Self> {
        let lower = s.to_lowercase();
        if lower.contains("callout-note") {
            Some(Self::Note)
        } else if lower.contains("callout-tip") {
            Some(Self::Tip)
        } else if lower.contains("callout-warning") {
            Some(Self::Warning)
        } else if lower.contains("callout-important") {
            Some(Self::Important)
        } else if lower.contains("callout-caution") {
            Some(Self::Caution)
        } else {
            None
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Note => "Nota",
            Self::Tip => "Consello",
            Self::Warning => "Aviso",
            Self::Important => "Importante",
            Self::Caution => "Precaución",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Note => "ℹ",
            Self::Tip => "💡",
            Self::Warning => "⚠",
            Self::Important => "❗",
            Self::Caution => "⚡",
        }
    }

    pub fn colors(&self, is_dark: bool) -> (Color32, Color32, Color32) {
        if is_dark {
            match self {
                Self::Note => (
                    Color32::from_rgb(0x40, 0x9E, 0xFF),
                    Color32::from_rgb(0x18, 0x2A, 0x3E),
                    Color32::from_rgb(0x8A, 0xC5, 0xFF),
                ),
                Self::Tip => (
                    Color32::from_rgb(0x27, 0xC9, 0x3F),
                    Color32::from_rgb(0x18, 0x34, 0x20),
                    Color32::from_rgb(0x7A, 0xE8, 0x8C),
                ),
                Self::Warning => (
                    Color32::from_rgb(0xFF, 0xBD, 0x2E),
                    Color32::from_rgb(0x3B, 0x30, 0x14),
                    Color32::from_rgb(0xFF, 0xD7, 0x7A),
                ),
                Self::Important => (
                    Color32::from_rgb(0xFF, 0x5F, 0x56),
                    Color32::from_rgb(0x3B, 0x1B, 0x1B),
                    Color32::from_rgb(0xFF, 0x99, 0x94),
                ),
                Self::Caution => (
                    Color32::from_rgb(0xFF, 0x8C, 0x3B),
                    Color32::from_rgb(0x3E, 0x24, 0x14),
                    Color32::from_rgb(0xFF, 0xB6, 0x82),
                ),
            }
        } else {
            match self {
                Self::Note => (
                    Color32::from_rgb(0x09, 0x69, 0xDA),
                    Color32::from_rgb(0xEE, 0xF5, 0xFF),
                    Color32::from_rgb(0x09, 0x69, 0xDA),
                ),
                Self::Tip => (
                    Color32::from_rgb(0x1A, 0x7F, 0x37),
                    Color32::from_rgb(0xEF, 0xF9, 0xF0),
                    Color32::from_rgb(0x1A, 0x7F, 0x37),
                ),
                Self::Warning => (
                    Color32::from_rgb(0xBF, 0x87, 0x00),
                    Color32::from_rgb(0xFF, 0xF8, 0xDC),
                    Color32::from_rgb(0x9A, 0x67, 0x00),
                ),
                Self::Important => (
                    Color32::from_rgb(0xCF, 0x22, 0x2E),
                    Color32::from_rgb(0xFF, 0xF0, 0xF1),
                    Color32::from_rgb(0xCF, 0x22, 0x2E),
                ),
                Self::Caution => (
                    Color32::from_rgb(0xBC, 0x4C, 0x00),
                    Color32::from_rgb(0xFF, 0xF3, 0xEA),
                    Color32::from_rgb(0xBC, 0x4C, 0x00),
                ),
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct QuartoCodeChunk {
    pub lang: String,
    pub options: Vec<(String, String)>,
    pub code: String,
    pub output_preview: Option<String>,
}

pub fn parse_quarto_chunk(lang_str: &str, code_text: &str) -> QuartoCodeChunk {
    let clean_lang = lang_str
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .split(',')
        .next()
        .unwrap_or("python")
        .trim()
        .to_string();

    let mut options = Vec::new();
    let mut code_lines = Vec::new();

    for line in code_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#|") {
            let opt_part = trimmed.trim_start_matches("#|").trim();
            if let Some((k, v)) = opt_part.split_once(':') {
                options.push((k.trim().to_string(), v.trim().to_string()));
            } else {
                options.push((opt_part.to_string(), String::new()));
            }
        } else {
            code_lines.push(line);
        }
    }

    let code = code_lines.join("\n");

    let mut output_preview = None;
    if code.contains("print(") || code.contains("summary(") || code.contains("plot(") || code.contains("head(") {
        let mut sim_out = Vec::new();
        for line in code.lines() {
            if line.contains("print(") {
                if let Some(start) = line.find("print(") {
                    let inside = &line[start + 6..];
                    if let Some(end) = inside.rfind(')') {
                        let text = inside[..end].trim_matches('"').trim_matches('\'').trim_start_matches('f').trim_matches('"');
                        sim_out.push(format!(">>> {}", text));
                    }
                }
            } else if line.contains("summary(") {
                sim_out.push("   Min. 1st Qu.  Median    Mean 3rd Qu.    Max. \n  12.00   23.00   50.50   48.67   67.00   89.00".to_string());
            }
        }
        if !sim_out.is_empty() {
            output_preview = Some(sim_out.join("\n"));
        } else if code.contains("plot(") {
            output_preview = Some("[Gráfico xerado por Quarto / Figure rendered]".to_string());
        }
    }

    QuartoCodeChunk {
        lang: clean_lang,
        options,
        code,
        output_preview,
    }
}

impl Drop for PythonKernel {
    fn drop(&mut self) {
        // Dropping the session sends EXIT and kills the interpreter.
        self.session = None;
        let _ = std::fs::remove_dir_all(&self.tmp_root);
    }
}

#[derive(Clone, Debug)]
pub struct CellExecutionResult {
    pub execution_id: usize,
    pub stdout: String,
    pub stderr: String,
    pub execution_time_secs: f64,
    pub success: bool,
    pub plot_image_path: Option<String>,
}

struct DetectedPython {
    binary: Option<String>,
    version: String,
    numpy: bool,
    matplotlib: bool,
}

/// Every plausible interpreter, ordered by how likely it is to be the user's
/// intended one. The first version of this app only tried `python3`, which on
/// Homebrew/macOS is often a bare interpreter without NumPy or Matplotlib.
fn python_candidates() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    #[cfg(unix)]
    {
        if let Ok(output) = std::process::Command::new("which")
            .args(["-a", "python3", "python"])
            .output()
        {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let path = line.trim();
                if !path.is_empty() {
                    out.push(path.to_string());
                }
            }
        }
    }
    #[cfg(windows)]
    {
        if let Ok(output) = std::process::Command::new("where")
            .args(["python3", "python", "py"])
            .output()
        {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let path = line.trim();
                if !path.is_empty() {
                    out.push(path.to_string());
                }
            }
        }
    }

    for extra in [
        "python3",
        "python",
        "/usr/bin/python3",
        "/usr/local/bin/python3",
        "/opt/homebrew/bin/python3",
        "/opt/local/bin/python3",
        "py",
    ] {
        out.push(extra.to_string());
    }

    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// One cell waiting to be executed by the persistent Python process.
struct QueuedCell {
    key: String,
    code: String,
    cwd: Option<PathBuf>,
}

struct ActiveExec {
    key: String,
    job_id: usize,
    plot_path: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    started: Instant,
}

/// A long-lived `python3` process that keeps variables, imports and figures
/// alive between cells — like a Jupyter kernel, which is what the UI promises.
struct KernelSession {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    lines: Receiver<String>,
}

impl Drop for KernelSession {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "EXIT");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Python driver fed over stdin. Each request names temporary files instead of
/// passing code inline, so any Python source round-trips without escaping.
const DRIVER: &str = r#"
import sys, os, time, traceback, contextlib

real_out = sys.stdout
ns = {"__name__": "__main__", "__builtins__": __builtins__}


def capture_plot(path):
    """Save any open Matplotlib figures. Matplotlib is imported lazily so the
    driver starts instantly and an interrupt cannot kill it during startup."""
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        import warnings
        warnings.filterwarnings("ignore", message=".*non-interactive.*")
        if plt.get_fignums():
            plt.savefig(path, bbox_inches="tight", dpi=110)
            plt.close("all")
    except BaseException:
        pass


while True:
    try:
        line = sys.stdin.readline()
    except BaseException:
        continue
    if not line:
        break
    line = line.rstrip("\r\n")
    if line == "EXIT":
        break
    parts = line.split("\t")
    if parts[0] != "EXEC" or len(parts) < 6:
        continue
    job_id, code_path, out_path, err_path, plot_path = parts[1:6]
    workdir = parts[6] if len(parts) > 6 else ""
    if workdir:
        try:
            os.chdir(workdir)
        except Exception:
            pass
    started = time.time()
    ok = True
    try:
        with open(code_path, "r", encoding="utf-8") as fh:
            code = fh.read()
        with open(out_path, "w", encoding="utf-8") as fo, \
             open(err_path, "w", encoding="utf-8") as fe:
            with contextlib.redirect_stdout(fo), contextlib.redirect_stderr(fe):
                try:
                    exec(compile(code, "<cell %s>" % job_id, "exec"), ns)
                except BaseException:
                    ok = False
                    traceback.print_exc()
    except BaseException:
        ok = False
    capture_plot(plot_path)
    try:
        real_out.write("@@DONE\t%s\t%s\t%.4f\n" % (job_id, "1" if ok else "0", time.time() - started))
        real_out.flush()
    except BaseException:
        pass
"#;

pub struct PythonKernel {
    pub execution_counter: usize,
    pub cell_outputs: HashMap<String, CellExecutionResult>,
    pub textures: HashMap<String, egui::TextureHandle>,
    pub python_binary: Option<String>,
    pub python_version: String,
    pub has_numpy: bool,
    pub has_matplotlib: bool,

    session: Option<KernelSession>,
    queue: VecDeque<QueuedCell>,
    active: Option<ActiveExec>,
    work_dir: Option<PathBuf>,
    /// Per-kernel scratch directory; unique per process so parallel kernels (or
    /// tests) never overwrite each other's code/output files.
    tmp_root: PathBuf,
    timeout: Duration,
    pub last_session_error: Option<String>,
}

impl Default for PythonKernel {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonKernel {
    pub fn new() -> Self {
        let detected = Self::detect_python();
        Self {
            execution_counter: 0,
            cell_outputs: HashMap::new(),
            textures: HashMap::new(),
            python_binary: detected.binary,
            python_version: detected.version,
            has_numpy: detected.numpy,
            has_matplotlib: detected.matplotlib,
            session: None,
            queue: VecDeque::new(),
            active: None,
            work_dir: None,
            tmp_root: Self::unique_tmp_root(),
            timeout: Duration::from_secs(180),
            last_session_error: None,
        }
    }

    fn detect_python() -> DetectedPython {
        let candidates = python_candidates();
        let mut fallback: Option<DetectedPython> = None;

        for candidate in candidates {
            let Ok(version_output) = std::process::Command::new(&candidate).arg("--version").output() else {
                continue;
            };
            if !version_output.status.success() {
                continue;
            }
            let version = {
                let stdout = String::from_utf8_lossy(&version_output.stdout).trim().to_string();
                if stdout.is_empty() {
                    String::from_utf8_lossy(&version_output.stderr).trim().to_string()
                } else {
                    stdout
                }
            };

            // Detect libraries without importing them (fast).
            let probe = std::process::Command::new(&candidate)
                .args([
                    "-c",
                    "import importlib.util as u; print(int(u.find_spec('numpy') is not None), int(u.find_spec('matplotlib') is not None))",
                ])
                .output();
            let (numpy, matplotlib) = probe
                .ok()
                .filter(|o| o.status.success())
                .map(|o| {
                    let text = String::from_utf8_lossy(&o.stdout).to_string();
                    let mut parts = text.split_whitespace();
                    let n = parts.next().map(|v| v == "1").unwrap_or(false);
                    let m = parts.next().map(|v| v == "1").unwrap_or(false);
                    (n, m)
                })
                .unwrap_or((false, false));

            let detected = DetectedPython {
                binary: Some(candidate.clone()),
                version,
                numpy,
                matplotlib,
            };

            // Prefer an interpreter with the scientific stack the UI advertises.
            if numpy && matplotlib {
                return detected;
            }
            if numpy && fallback.is_none() {
                fallback = Some(detected);
            } else if fallback.is_none() {
                fallback = Some(detected);
            }
        }

        fallback.unwrap_or(DetectedPython {
            binary: None,
            version: "Python non dispoñible".to_string(),
            numpy: false,
            matplotlib: false,
        })
    }

    pub fn is_busy(&self) -> bool {
        self.active.is_some() || !self.queue.is_empty()
    }

    pub fn is_running_cell(&self, key: &str) -> bool {
        self.active.as_ref().is_some_and(|a| a.key == key)
    }

    fn unique_tmp_root() -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "tigorra-kernel-{}-{}-{}",
            std::process::id(),
            n,
            nanos
        ))
    }

    fn tmp_dir(&self) -> PathBuf {
        self.tmp_root.clone()
    }

    fn ensure_session(&mut self, cwd: Option<&Path>) -> Result<(), String> {
        if let Some(session) = self.session.as_mut() {
            if session.child.try_wait().ok().flatten().is_none() {
                return Ok(());
            }
            // Process died: drop it and start a fresh kernel.
            self.session = None;
        }

        let Some(binary) = self.python_binary.clone() else {
            return Err("Python 3 non está instalado ou non se atopa no PATH".to_string());
        };

        let dir = self.tmp_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Non se puido crear {}: {}", dir.display(), e))?;

        let mut command = std::process::Command::new(&binary);
        command
            .args(["-u", "-c", DRIVER])
            .env("MPLBACKEND", "Agg")
            .env("PYTHONIOENCODING", "utf-8")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        if let Some(cwd) = cwd {
            if cwd.is_dir() {
                command.current_dir(cwd);
            }
        }

        let mut child = command
            .spawn()
            .map_err(|e| format!("Non se puido iniciar {}: {}", binary, e))?;

        let stdin = child.stdin.take().ok_or("Sen entrada estándar para o kernel")?;
        let stdout = child.stdout.take().ok_or("Sen saída estándar para o kernel")?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        self.session = Some(KernelSession { child, stdin, lines: rx });
        self.last_session_error = None;
        Ok(())
    }

    /// Queues a cell. Execution happens in the background and results appear in
    /// `cell_outputs` after `poll` is called.
    pub fn request_execution(&mut self, key: &str, code: &str, cwd: Option<&Path>) {
        self.queue.push_back(QueuedCell {
            key: key.to_string(),
            code: code.to_string(),
            cwd: cwd.map(|p| p.to_path_buf()),
        });
    }

    pub fn execute_all_cells(&mut self, content: &str, cwd: Option<&Path>) -> usize {
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;
        let mut cell_idx = 0;
        let mut queued = 0;

        while i < lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with("```") {
                let lang = trimmed.trim_start_matches('`').trim();
                let mut code_lines = Vec::new();
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with("```") {
                    code_lines.push(lines[i]);
                    i += 1;
                }
                if i < lines.len() {
                    i += 1;
                }
                if lang.starts_with('{') && lang.ends_with('}') {
                    let clean_lang = lang
                        .trim_start_matches('{')
                        .trim_end_matches('}')
                        .split(',')
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();
                    if clean_lang.starts_with("python") || clean_lang == "py" {
                        let chunk = parse_quarto_chunk(lang, &code_lines.join("\n"));
                        let cell_key = format!("cell_{}_python", cell_idx);
                        self.request_execution(&cell_key, &chunk.code, cwd);
                        queued += 1;
                    }
                    cell_idx += 1;
                }
                continue;
            }
            i += 1;
        }

        queued
    }

    fn submit_next(&mut self) {
        if self.active.is_some() {
            return;
        }
        let Some(job) = self.queue.pop_front() else {
            return;
        };
        if let Err(e) = self.ensure_session(job.cwd.as_deref()) {
            #[cfg(test)]
            eprintln!("[kernel] ensure_session failed: {}", e);
            self.last_session_error = Some(e.clone());
            self.cell_outputs.insert(
                job.key.clone(),
                CellExecutionResult {
                    execution_id: self.execution_counter,
                    stdout: String::new(),
                    stderr: e,
                    execution_time_secs: 0.0,
                    success: false,
                    plot_image_path: None,
                },
            );
            return;
        }

        self.execution_counter += 1;
        let job_id = self.execution_counter;
        let dir = self.tmp_dir();
        let _ = std::fs::create_dir_all(&dir);

        let code_path = dir.join(format!("cell_{}.py", job_id));
        let stdout_path = dir.join(format!("cell_{}.out", job_id));
        let stderr_path = dir.join(format!("cell_{}.err", job_id));
        let plot_path = dir.join(format!("cell_{}.png", job_id));
        let _ = std::fs::remove_file(&plot_path);

        if std::fs::write(&code_path, job.code.as_bytes()).is_err() {
            #[cfg(test)]
            eprintln!("[kernel] write code failed: {}", code_path.display());
            self.last_session_error = Some("Non se puido escribir o código da cela".to_string());
            return;
        }

        let Some(session) = self.session.as_mut() else {
            #[cfg(test)]
            eprintln!("[kernel] no session after ensure_session");
            return;
        };
        let cwd = job
            .cwd
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let request = format!(
            "EXEC\t{}\t{}\t{}\t{}\t{}\t{}\n",
            job_id,
            code_path.display(),
            stdout_path.display(),
            stderr_path.display(),
            plot_path.display(),
            cwd
        );
        if let Err(e) = session.stdin.write_all(request.as_bytes()).and_then(|_| session.stdin.flush()) {
            self.last_session_error = Some(format!("Erro ao enviar a cela ao kernel: {}", e));
            self.session = None;
            return;
        }

        self.work_dir = job.cwd.clone();
        self.active = Some(ActiveExec {
            key: job.key,
            job_id,
            plot_path,
            stdout_path,
            stderr_path,
            started: Instant::now(),
        });
    }

    /// Drains finished jobs, loads their output and starts the next queued cell.
    /// Must be called once per frame; it never blocks.
    pub fn poll(&mut self, ctx: Option<&egui::Context>) {
        if self.is_busy() {
            if let Some(ctx) = ctx {
                ctx.request_repaint_after(Duration::from_millis(80));
            }
        }
        self.submit_next();

        let mut finished: Vec<(usize, bool, f64)> = Vec::new();
        if let Some(session) = self.session.as_ref() {
            while let Ok(line) = session.lines.try_recv() {
                if let Some(rest) = line.strip_prefix("@@DONE\t") {
                    let fields: Vec<&str> = rest.split('\t').collect();
                    if fields.len() >= 3 {
                        let id = fields[0].parse::<usize>().unwrap_or(0);
                        let ok = fields[1] == "1";
                        let secs = fields[2].parse::<f64>().unwrap_or(0.0);
                        finished.push((id, ok, secs));
                    }
                }
            }
        }

        // If the interpreter died (crash, OOM, early SIGINT) do not leave the
        // UI spinning forever: report it as a failed cell.
        let exited = self
            .session
            .as_mut()
            .and_then(|session| session.child.try_wait().ok().flatten());
        if let Some(status) = exited {
            if let Some(active) = self.active.take() {
                let stderr = std::fs::read_to_string(&active.stderr_path).unwrap_or_default();
                let detail = if stderr.trim().is_empty() {
                    format!("O kernel de Python finalizou inesperadamente ({})", status)
                } else {
                    format!("O kernel finalizou ({}):\n{}", status, stderr)
                };
                self.session = None;
                self.last_session_error = Some(detail.clone());
                self.cell_outputs.insert(
                    active.key,
                    CellExecutionResult {
                        execution_id: active.job_id,
                        stdout: std::fs::read_to_string(&active.stdout_path).unwrap_or_default(),
                        stderr: detail,
                        execution_time_secs: active.started.elapsed().as_secs_f64(),
                        success: false,
                        plot_image_path: None,
                    },
                );
            }
        }

        for (id, ok, secs) in finished {
            let Some(active) = self.active.take() else { break };
            if active.job_id != id {
                self.active = Some(active);
                continue;
            }
            let stdout = std::fs::read_to_string(&active.stdout_path).unwrap_or_default();
            let stderr = std::fs::read_to_string(&active.stderr_path).unwrap_or_default();
            let plot = if active.plot_path.exists() { Some(active.plot_path.to_string_lossy().to_string()) } else { None };

            if let (Some(ctx), Some(path)) = (ctx, plot.as_deref()) {
                self.load_plot_texture(ctx, path);
            }

            self.cell_outputs.insert(
                active.key.clone(),
                CellExecutionResult {
                    execution_id: id,
                    stdout,
                    stderr,
                    execution_time_secs: secs,
                    success: ok,
                    plot_image_path: plot,
                },
            );
        }

        // Timeout: kill the kernel so a runaway loop cannot hang the app.
        if let Some(active) = self.active.as_ref() {
            if active.started.elapsed() > self.timeout {
                let key = active.key.clone();
                let id = active.job_id;
                self.session = None;
                self.active = None;
                self.last_session_error = Some(format!(
                    "A cela tardou máis de {}s e o kernel reiniciouse",
                    self.timeout.as_secs()
                ));
                self.cell_outputs.insert(
                    key,
                    CellExecutionResult {
                        execution_id: id,
                        stdout: String::new(),
                        stderr: format!(
                            "⏱ Tempo de execución excedido ({}s). O kernel reiniciouse; volve executar a cela.",
                            self.timeout.as_secs()
                        ),
                        execution_time_secs: self.timeout.as_secs_f64(),
                        success: false,
                        plot_image_path: None,
                    },
                );
            }
        }

        self.submit_next();
    }

    /// Sends SIGINT to the kernel so a runaway cell stops *without* losing the
    /// session state (Python raises `KeyboardInterrupt`, which the driver
    /// reports as a failed cell while keeping the interpreter alive).
    pub fn interrupt(&mut self) -> bool {
        let Some(session) = self.session.as_ref() else {
            return false;
        };
        let pid = session.child.id().to_string();
        let status = if cfg!(windows) {
            std::process::Command::new("taskkill")
                .args(["/PID", &pid, "/F"])
                .status()
        } else {
            std::process::Command::new("kill")
                .args(["-INT", &pid])
                .status()
        };
        let sent = status.map(|s| s.success()).unwrap_or(false);
        if sent {
            self.last_session_error = Some("Interrupción enviada ao kernel".to_string());
        }
        sent
    }

    pub fn clear_outputs(&mut self) {
        self.execution_counter = 0;
        self.cell_outputs.clear();
        self.textures.clear();
        self.queue.clear();
        self.session = None;
        self.active = None;
        self.last_session_error = None;
        let _ = std::fs::remove_dir_all(self.tmp_dir());
    }

    pub fn load_plot_texture(
        &mut self,
        ctx: &egui::Context,
        path_str: &str,
    ) -> Option<egui::TextureHandle> {
        if let Some(tex) = self.textures.get(path_str) {
            return Some(tex.clone());
        }

        let bytes = std::fs::read(path_str).ok()?;
        let img = image::load_from_memory(&bytes).ok()?;
        let rgba = img.to_rgba8();
        let (width, height) = (rgba.width() as usize, rgba.height() as usize);
        let pixels = rgba.into_raw();
        let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);
        let texture = ctx.load_texture(path_str, color_image, egui::TextureOptions::LINEAR);
        self.textures.insert(path_str.to_string(), texture.clone());
        Some(texture)
    }

    /// Blocking helper used by tests and by headless renders: runs one cell and
    /// waits until the kernel reports the result.
    #[allow(dead_code)]
    pub fn execute_cell_blocking(
        &mut self,
        key: &str,
        code: &str,
        cwd: Option<&Path>,
    ) -> CellExecutionResult {
        self.request_execution(key, code, cwd);
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            self.poll(None);
            if !self.is_busy() {
                if let Some(result) = self.cell_outputs.get(key) {
                    return result.clone();
                }
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        self.poll(None);
        self.cell_outputs.get(key).cloned().unwrap_or(CellExecutionResult {
            execution_id: self.execution_counter,
            stdout: String::new(),
            stderr: "⏱ A execución non rematou a tempo".to_string(),
            execution_time_secs: 60.0,
            success: false,
            plot_image_path: None,
        })
    }
}

pub fn render_quarto_frontmatter_ui(
    ui: &mut Ui,
    meta: &QuartoMetadata,
    theme: &ColorPalette,
    is_quarto_doc: bool,
) {
    let frame = Frame::none()
        .fill(theme.quote_bg)
        .rounding(Rounding::same(8.0_f32))
        .stroke(Stroke::new(1.5_f32, Color32::from_rgb(0x40, 0x9E, 0xFF)))
        .inner_margin(Margin::same(16.0_f32));

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            // Quarto badge
            let badge_frame = Frame::none()
                .fill(Color32::from_rgb(0x40, 0x9E, 0xFF))
                .rounding(Rounding::same(4.0_f32))
                .inner_margin(Margin::symmetric(6.0_f32, 2.0_f32));
            badge_frame.show(ui, |ui| {
                let badge = if is_quarto_doc { "📊 QUARTO (.QMD)" } else { "🏷 METADATOS YAML" };
                ui.label(RichText::new(badge).size(10.5).strong().color(Color32::WHITE));
            });

            // Format badge
            let fmt_frame = Frame::none()
                .fill(theme.code_bg)
                .rounding(Rounding::same(4.0_f32))
                .stroke(Stroke::new(1.0_f32, theme.border))
                .inner_margin(Margin::symmetric(6.0_f32, 2.0_f32));
            fmt_frame.show(ui, |ui| {
                ui.label(RichText::new(format!("format: {}", meta.format)).size(10.5).monospace().color(theme.text_muted));
            });
        });

        ui.add_space(8.0);

        if let Some(ref title) = meta.title {
            ui.label(
                RichText::new(title)
                    .size(24.0)
                    .strong()
                    .color(theme.text_primary),
            );
        }

        if let Some(ref subtitle) = meta.subtitle {
            ui.add_space(2.0);
            ui.label(
                RichText::new(subtitle)
                    .size(15.0)
                    .italics()
                    .color(theme.text_muted),
            );
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if let Some(ref author) = meta.author {
                ui.label(RichText::new(format!("👤 {}", author)).size(12.5).color(theme.accent));
                ui.add_space(10.0);
            }
            if let Some(ref date) = meta.date {
                ui.label(RichText::new(format!("📅 {}", date)).size(12.5).color(theme.text_muted));
            }
        });

        if !meta.raw_yaml.is_empty() {
            ui.add_space(6.0);
            ui.collapsing("Ver metadatos YAML orixinais", |ui| {
                ui.label(
                    RichText::new(&meta.raw_yaml)
                        .font(FontId::monospace(11.5))
                        .color(theme.text_muted),
                );
            });
        }
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);
}

pub fn render_quarto_callout_ui(
    ui: &mut Ui,
    callout_type: QuartoCalloutType,
    lines: &[&str],
    theme: &ColorPalette,
    _stylesheet: TyporaStylesheet,
    _math_renderer: &mut MathRenderer,
) {
    let (border_col, bg_col, text_col) = callout_type.colors(theme.is_dark);

    let frame = Frame::none()
        .fill(bg_col)
        .rounding(Rounding::same(6.0_f32))
        .stroke(Stroke::new(1.0_f32, border_col.linear_multiply(0.4)))
        .inner_margin(Margin::same(12.0_f32));

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            // Left thick indicator line
            let (rect, _response) = ui.allocate_exact_size(Vec2::new(4.0, 24.0), Sense::hover());
            ui.painter().rect_filled(rect, Rounding::same(2.0_f32), border_col);

            ui.add_space(6.0);

            // Icon and title
            ui.label(RichText::new(callout_type.icon()).size(15.0));
            ui.add_space(2.0);

            let first_line = lines.first().map(|s| s.trim()).unwrap_or("");
            let title = if first_line.starts_with('#') {
                first_line.trim_start_matches('#').trim()
            } else {
                callout_type.name()
            };

            ui.label(
                RichText::new(title)
                    .size(13.5)
                    .strong()
                    .color(text_col),
            );
        });

        ui.add_space(6.0);

        // Body content
        let start_line = if lines.first().map(|s| s.trim().starts_with('#')).unwrap_or(false) {
            1
        } else {
            0
        };

        for &line in &lines[start_line..] {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                ui.add_space(4.0);
                continue;
            }
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(
                    RichText::new(trimmed)
                        .size(13.0)
                        .color(theme.text_primary),
                );
            });
        }
    });
}

pub fn render_quarto_chunk_ui(
    ui: &mut Ui,
    chunk: &QuartoCodeChunk,
    cell_idx: usize,
    theme: &ColorPalette,
    kernel: &mut PythonKernel,
    doc_dir: Option<&Path>,
) {
    let is_python = chunk.lang == "python" || chunk.lang == "py";
    let cell_key = format!("cell_{}_{}", cell_idx, chunk.lang);
    let maybe_res = kernel.cell_outputs.get(&cell_key).cloned();

    let frame = Frame::none()
        .fill(theme.code_bg)
        .rounding(Rounding::same(6.0_f32))
        .stroke(Stroke::new(
            if maybe_res.is_some() { 1.5_f32 } else { 1.0_f32 },
            if is_python {
                Color32::from_rgb(0x40, 0x9E, 0xFF)
            } else {
                theme.border
            },
        ))
        .inner_margin(Margin::same(10.0_f32));

    frame.show(ui, |ui| {
        // Jupyter Prompt & Header
        ui.horizontal(|ui| {
            // Execution counter badge: In [1]: or In [ ]:
            let (exec_badge, badge_color) = if let Some(ref r) = maybe_res {
                if r.success {
                    (format!("In [{}]:", r.execution_id), Color32::from_rgb(0x27, 0xC9, 0x3F))
                } else {
                    (format!("In [{}]:", r.execution_id), Color32::from_rgb(0xFF, 0x5F, 0x56))
                }
            } else {
                ("In [ ]:".to_string(), theme.text_muted)
            };

            ui.label(RichText::new(exec_badge).font(FontId::monospace(12.0)).strong().color(badge_color));

            // Language badge
            if is_python {
                ui.label(
                    RichText::new("🐍 Python (Jupyter)")
                        .font(FontId::monospace(11.5))
                        .strong()
                        .color(Color32::from_rgb(0x40, 0x9E, 0xFF)),
                );
            } else {
                ui.label(
                    RichText::new(format!("▶ Quarto Chunk: {}", chunk.lang))
                        .font(FontId::monospace(11.5))
                        .strong()
                        .color(Color32::from_rgb(0x40, 0x9E, 0xFF)),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.button(RichText::new("Copiar").size(10.5)).clicked() {
                    ui.output_mut(|o| o.copied_text = chunk.code.clone());
                }

                if is_python {
                    let running = kernel.is_running_cell(&cell_key);
                    if running {
                        let stop = ui.add(
                            Button::new(
                                RichText::new("⏹ Interrumpir")
                                    .size(11.0)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(0xB0, 0x3A, 0x3A))
                            .rounding(Rounding::same(4.0_f32)),
                        ).on_hover_text("Enviar SIGINT ao kernel sen perder o estado");
                        if stop.clicked() {
                            kernel.interrupt();
                        }
                    }
                    let run_btn = ui.add_enabled(
                        !running,
                        Button::new(
                            RichText::new(if running { "⏳ Executando…" } else { "▶ Executar cela" })
                                .size(11.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(if running { Color32::from_rgb(0x5A, 0x6A, 0x7A) } else { Color32::from_rgb(0x27, 0xC9, 0x3F) })
                        .rounding(Rounding::same(4.0_f32)),
                    );
                    if run_btn.clicked() {
                        kernel.request_execution(&cell_key, &chunk.code, doc_dir);
                    }
                }
            });
        });

        // Chunk Options Tags (e.g. #| label: ..., #| fig-cap: ...)
        if !chunk.options.is_empty() {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (k, v) in &chunk.options {
                    let opt_frame = Frame::none()
                        .fill(theme.quote_bg)
                        .rounding(Rounding::same(3.0_f32))
                        .stroke(Stroke::new(1.0_f32, theme.border))
                        .inner_margin(Margin::symmetric(4.0_f32, 1.0_f32));
                    opt_frame.show(ui, |ui| {
                        let text = if v.is_empty() {
                            format!("#| {}", k)
                        } else {
                            format!("#| {}: {}", k, v)
                        };
                        ui.label(RichText::new(text).size(10.0).monospace().color(theme.text_muted));
                    });
                }
            });
        }

        ui.add_space(6.0);

        // Code body
        egui::ScrollArea::horizontal()
            .id_salt(egui::Id::new("quarto_chunk_scroll").with(&cell_key).with(&chunk.code))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(&chunk.code)
                        .font(FontId::monospace(13.0))
                        .color(theme.text_primary),
                );
            });

        // Jupyter Output Section
        if let Some(ref res) = maybe_res {
            ui.add_space(8.0);
            let out_frame = Frame::none()
                .fill(theme.bg_color)
                .rounding(Rounding::same(6.0_f32))
                .stroke(Stroke::new(
                    1.0_f32,
                    if res.success {
                        Color32::from_rgb(0x27, 0xC9, 0x3F)
                    } else {
                        Color32::from_rgb(0xFF, 0x5F, 0x56)
                    },
                ))
                .inner_margin(Margin::same(10.0_f32));

            out_frame.show(ui, |ui| {
                // Out [1]: header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Out [{}]:", res.execution_id))
                            .font(FontId::monospace(11.5))
                            .strong()
                            .color(if res.success {
                                Color32::from_rgb(0x27, 0xC9, 0x3F)
                            } else {
                                Color32::from_rgb(0xFF, 0x5F, 0x56)
                            }),
                    );
                    ui.label(
                        RichText::new(format!("⚡ {:.3}s", res.execution_time_secs))
                            .size(10.5)
                            .color(theme.text_muted),
                    );
                });

                // Standard Output (print)
                if !res.stdout.is_empty() {
                    ui.add_space(4.0);
                    egui::ScrollArea::horizontal()
                        .id_salt(egui::Id::new("cell_stdout").with(&cell_key))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(&res.stdout)
                                    .font(FontId::monospace(12.5))
                                    .color(theme.text_body),
                            );
                        });
                }

                // Error Traceback (stderr)
                if !res.stderr.is_empty() {
                    ui.add_space(4.0);
                    let err_frame = Frame::none()
                        .fill(Color32::from_rgb(0x3B, 0x1B, 0x1B))
                        .rounding(Rounding::same(4.0_f32))
                        .inner_margin(Margin::same(6.0_f32));
                    err_frame.show(ui, |ui| {
                        ui.label(
                            RichText::new(&res.stderr)
                                .font(FontId::monospace(11.5))
                                .color(Color32::from_rgb(0xFF, 0x99, 0x94)),
                        );
                    });
                }

                // Plot Image Display (Matplotlib)
                if let Some(ref plot_path) = res.plot_image_path {
                    ui.add_space(8.0);
                    let ctx = ui.ctx().clone();
                    if let Some(tex) = kernel.load_plot_texture(&ctx, plot_path) {
                        ui.label(RichText::new("📊 Gráfico xerado por Matplotlib:").size(11.0).strong().color(theme.accent));
                        ui.add_space(4.0);
                        let avail_w = (ui.available_width() - 10.0).clamp(200.0, 680.0);
                        let img_widget = egui::Image::new(&tex).max_width(avail_w).rounding(Rounding::same(4.0));
                        ui.add(img_widget);
                    }
                }
            });
        } else if let Some(ref out) = chunk.output_preview {
            // Simulated / hint preview if not yet executed
            ui.add_space(6.0);
            let out_frame = Frame::none()
                .fill(theme.bg_color)
                .rounding(Rounding::same(4.0_f32))
                .stroke(Stroke::new(1.0_f32, theme.border))
                .inner_margin(Margin::same(8.0_f32));
            out_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📊 Saída estimada (preme '▶ Executar cela' para correr con Python):").size(10.5).color(theme.text_muted));
                });
                ui.add_space(2.0);
                ui.label(
                    RichText::new(out)
                        .font(FontId::monospace(12.0))
                        .color(theme.text_muted),
                );
            });
        }
    });
}

pub fn render_quarto_to_html(
    title: &str,
    content: &str,
    style: TyporaStylesheet,
    kernel: Option<&PythonKernel>,
) -> String {
    let (meta, start_body_idx) = if let Some((m, idx)) = parse_quarto_frontmatter(content) {
        (m, idx)
    } else {
        (
            QuartoMetadata {
                title: Some(title.to_string()),
                ..Default::default()
            },
            0,
        )
    };

    let bg_color = match style {
        TyporaStylesheet::Night => "#1E2227",
        TyporaStylesheet::Newsprint => "#F7F6F2",
        TyporaStylesheet::GitHub => "#FFFFFF",
        TyporaStylesheet::Gothic => "#FFFFFF",
        TyporaStylesheet::Whitey => "#FAFAFA",
    };

    let text_color = match style {
        TyporaStylesheet::Night => "#ABB2BF",
        TyporaStylesheet::Newsprint => "#1A1A1A",
        TyporaStylesheet::GitHub => "#24292E",
        TyporaStylesheet::Gothic => "#111111",
        TyporaStylesheet::Whitey => "#2C3E50",
    };

    let code_bg = match style {
        TyporaStylesheet::Night => "#282C34",
        TyporaStylesheet::Newsprint => "#ECE8DE",
        TyporaStylesheet::GitHub => "#F6F8FA",
        TyporaStylesheet::Gothic => "#EFEFEF",
        TyporaStylesheet::Whitey => "#F0F3F4",
    };

    let display_title = meta.title.as_deref().unwrap_or(title);
    let author_html = meta
        .author
        .as_deref()
        .map(|a| format!("<div class=\"quarto-author\">👤 {}</div>", html_escape(a)))
        .unwrap_or_default();
    let date_html = meta
        .date
        .as_deref()
        .map(|d| format!("<div class=\"quarto-date\">📅 {}</div>", html_escape(d)))
        .unwrap_or_default();
    let subtitle_html = meta
        .subtitle
        .as_deref()
        .map(|s| format!("<div class=\"quarto-subtitle\">{}</div>", html_escape(s)))
        .unwrap_or_default();

    let lines: Vec<&str> = content.lines().collect();
    let body_lines = if start_body_idx < lines.len() {
        &lines[start_body_idx..]
    } else {
        &lines[..]
    };

    let mut body_html = String::new();
    let mut i = 0;
    let mut cell_idx = 0;

    while i < body_lines.len() {
        let line = body_lines[i];
        let trimmed = line.trim();

        // Check for Quarto Callout block
        if trimmed.starts_with(":::") {
            if let Some(callout_type) = QuartoCalloutType::from_str(trimmed) {
                let callout_cls = match callout_type {
                    QuartoCalloutType::Note => "callout-note",
                    QuartoCalloutType::Tip => "callout-tip",
                    QuartoCalloutType::Warning => "callout-warning",
                    QuartoCalloutType::Important => "callout-important",
                    QuartoCalloutType::Caution => "callout-caution",
                };

                let mut callout_inner = Vec::new();
                i += 1;
                while i < body_lines.len() && body_lines[i].trim() != ":::" {
                    callout_inner.push(body_lines[i]);
                    i += 1;
                }
                if i < body_lines.len() {
                    i += 1;
                }

                let first = callout_inner.first().map(|s| s.trim()).unwrap_or("");
                let (c_title, body_start) = if first.starts_with('#') {
                    (first.trim_start_matches('#').trim(), 1)
                } else {
                    (callout_type.name(), 0)
                };

                body_html.push_str(&format!(
                    "<div class=\"callout {}\">\n<div class=\"callout-header\"><span class=\"callout-icon\">{}</span> <strong>{}</strong></div>\n<div class=\"callout-body\">\n",
                    callout_cls, callout_type.icon(), html_escape(c_title)
                ));

                let mut ci = body_start;
                while ci < callout_inner.len() {
                    let cline = callout_inner[ci];
                    let ctrimmed = cline.trim();
                    if ctrimmed.starts_with("$$") {
                        if ctrimmed.len() > 4 && ctrimmed.ends_with("$$") {
                            let formula = &ctrimmed[2..ctrimmed.len() - 2];
                            body_html.push_str(&format!(
                                "<div class=\"math-block\">$${}$$</div>\n",
                                html_escape(formula.trim())
                            ));
                            ci += 1;
                        } else {
                            let mut math_lines = Vec::new();
                            let first = ctrimmed.trim_start_matches('$').trim();
                            if !first.is_empty() {
                                math_lines.push(first);
                            }
                            ci += 1;
                            while ci < callout_inner.len() && !callout_inner[ci].trim().ends_with("$$") {
                                math_lines.push(callout_inner[ci]);
                                ci += 1;
                            }
                            if ci < callout_inner.len() {
                                let last = callout_inner[ci].trim().trim_end_matches('$').trim();
                                if !last.is_empty() {
                                    math_lines.push(last);
                                }
                                ci += 1;
                            }
                            let formula = math_lines.join("\n");
                            body_html.push_str(&format!(
                                "<div class=\"math-block\">$${}$$</div>\n",
                                html_escape(formula.trim())
                            ));
                        }
                    } else if !ctrimmed.is_empty() {
                        body_html.push_str(&format!("<p>{}</p>\n", format_inline_html(ctrimmed)));
                        ci += 1;
                    } else {
                        ci += 1;
                    }
                }
                body_html.push_str("</div>\n</div>\n");
                continue;
            }
        }

        // Code block or Quarto chunk
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim();
            let mut code_lines = Vec::new();
            i += 1;
            while i < body_lines.len() && !body_lines[i].trim().starts_with("```") {
                code_lines.push(body_lines[i]);
                i += 1;
            }
            if i < body_lines.len() {
                i += 1;
            }

            let code_joined = code_lines.join("\n");
            if lang.starts_with('{') && lang.ends_with('}') {
                let chunk = parse_quarto_chunk(lang, &code_joined);
                let cell_key = format!("cell_{}_{}", cell_idx, chunk.lang);
                let executed_res = kernel.and_then(|k| k.cell_outputs.get(&cell_key));

                body_html.push_str("<div class=\"quarto-chunk\">\n");
                body_html.push_str(&format!(
                    "<div class=\"quarto-chunk-header\"><span class=\"chunk-badge\">In [{}]: {}</span></div>\n",
                    executed_res.map(|r| r.execution_id.to_string()).unwrap_or_else(|| " ".to_string()),
                    html_escape(&chunk.lang)
                ));

                if !chunk.options.is_empty() {
                    body_html.push_str("<div class=\"quarto-chunk-options\">\n");
                    for (k, v) in &chunk.options {
                        body_html.push_str(&format!(
                            "<span class=\"chunk-opt\">#| {}: {}</span>\n",
                            html_escape(k),
                            html_escape(v)
                        ));
                    }
                    body_html.push_str("</div>\n");
                }

                body_html.push_str(&format!(
                    "<pre><code class=\"language-{}\">\n{}\n</code></pre>\n",
                    html_escape(&chunk.lang),
                    html_escape(&chunk.code)
                ));

                // If executed in Python kernel, show real output and embedded plot!
                if let Some(res) = executed_res {
                    body_html.push_str("<div class=\"quarto-chunk-output\">\n");
                    body_html.push_str(&format!(
                        "<div class=\"output-label\">Out [{}]: <span class=\"output-time\">⚡ {:.3}s</span></div>\n",
                        res.execution_id, res.execution_time_secs
                    ));

                    if !res.stdout.is_empty() {
                        body_html.push_str(&format!(
                            "<pre class=\"output-stdout\"><code>{}</code></pre>\n",
                            html_escape(&res.stdout)
                        ));
                    }

                    if !res.stderr.is_empty() {
                        body_html.push_str(&format!(
                            "<pre class=\"output-stderr\"><code>{}</code></pre>\n",
                            html_escape(&res.stderr)
                        ));
                    }

                    if let Some(ref plot_path) = res.plot_image_path {
                        if let Ok(bytes) = std::fs::read(plot_path) {
                            let b64 = base64_encode(&bytes);
                            body_html.push_str(&format!(
                                "<div class=\"output-plot-container\"><img src=\"data:image/png;base64,{}\" class=\"output-plot-img\" alt=\"Gráfico Quarto Matplotlib\" /></div>\n",
                                b64
                            ));
                        }
                    }
                    body_html.push_str("</div>\n");
                } else if let Some(ref out) = chunk.output_preview {
                    body_html.push_str(&format!(
                        "<div class=\"quarto-chunk-output\"><div class=\"output-label\">Saída estimada:</div><pre><code>{}</code></pre></div>\n",
                        html_escape(out)
                    ));
                }

                body_html.push_str("</div>\n");
                cell_idx += 1;
            } else {
                body_html.push_str(&format!(
                    "<pre><code class=\"language-{}\">\n{}\n</code></pre>\n",
                    html_escape(lang),
                    html_escape(&code_joined)
                ));
            }
            continue;
        }

        // Display Math $$ (single-line or multiline block)
        if trimmed.starts_with("$$") {
            if trimmed.len() > 4 && trimmed.ends_with("$$") {
                let formula = &trimmed[2..trimmed.len() - 2];
                body_html.push_str(&format!(
                    "<div class=\"math-block\">$${}$$</div>\n",
                    html_escape(formula.trim())
                ));
                i += 1;
                continue;
            } else {
                let mut math_lines = Vec::new();
                let first = trimmed.trim_start_matches('$').trim();
                if !first.is_empty() {
                    math_lines.push(first);
                }
                i += 1;
                while i < body_lines.len() && !body_lines[i].trim().ends_with("$$") {
                    math_lines.push(body_lines[i]);
                    i += 1;
                }
                if i < body_lines.len() {
                    let last = body_lines[i].trim().trim_end_matches('$').trim();
                    if !last.is_empty() {
                        math_lines.push(last);
                    }
                    i += 1;
                }
                let formula = math_lines.join("\n");
                body_html.push_str(&format!(
                    "<div class=\"math-block\">$${}$$</div>\n",
                    html_escape(formula.trim())
                ));
                continue;
            }
        }

        // Headings
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed[level..].trim();
            body_html.push_str(&format!("<h{}>{}</h{}>\n", level, html_escape(text), level));
            i += 1;
            continue;
        }

        // Horizontal rule
        if trimmed == "---" {
            body_html.push_str("<hr />\n");
            i += 1;
            continue;
        }

        // Regular paragraphs
        if trimmed.is_empty() {
            body_html.push_str("<p></p>\n");
        } else {
            body_html.push_str(&format!("<p>{}</p>\n", format_inline_html(trimmed)));
        }
        i += 1;
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="gl">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title} — Quarto Preview</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.css">
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/contrib/mhchem.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/contrib/auto-render.min.js"></script>
<style>
  body {{
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
    background-color: {bg_color};
    color: {text_color};
    line-height: 1.65;
    margin: 0;
    padding: 40px 20px;
    font-size: 16px;
  }}
  .quarto-container {{
    max-width: 900px;
    margin: 0 auto;
  }}
  .quarto-header-card {{
    background: {code_bg};
    border-radius: 8px;
    padding: 24px;
    margin-bottom: 30px;
    border-left: 5px solid #409EFF;
    box-shadow: 0 2px 8px rgba(0,0,0,0.08);
  }}
  .quarto-badge {{
    display: inline-block;
    background: #409EFF;
    color: white;
    font-weight: 700;
    font-size: 11px;
    padding: 3px 8px;
    border-radius: 4px;
    margin-bottom: 12px;
  }}
  .quarto-title {{
    font-size: 2.3em;
    font-weight: 700;
    margin: 0 0 8px 0;
  }}
  .quarto-subtitle {{
    font-size: 1.25em;
    color: #6c757d;
    font-style: italic;
    margin-bottom: 12px;
  }}
  .quarto-meta {{
    display: flex;
    gap: 16px;
    font-size: 0.95em;
    color: #555;
    margin-top: 8px;
  }}
  .callout {{
    border-radius: 6px;
    margin: 1.5em 0;
    padding: 14px 18px;
    border-left: 4px solid #409EFF;
    background: rgba(64, 158, 255, 0.08);
  }}
  .callout-note {{ border-color: #0969DA; background: rgba(9, 105, 218, 0.08); }}
  .callout-tip {{ border-color: #1A7F37; background: rgba(26, 127, 55, 0.08); }}
  .callout-warning {{ border-color: #BF8700; background: rgba(191, 135, 0, 0.08); }}
  .callout-important {{ border-color: #CF222E; background: rgba(207, 34, 46, 0.08); }}
  .callout-caution {{ border-color: #BC4C00; background: rgba(188, 76, 0, 0.08); }}
  .callout-header {{
    font-weight: 600;
    margin-bottom: 6px;
  }}
  .callout-icon {{
    margin-right: 6px;
  }}
  .quarto-chunk {{
    background: {code_bg};
    border: 1px solid rgba(64, 158, 255, 0.3);
    border-radius: 6px;
    margin: 1.5em 0;
    overflow: hidden;
  }}
  .quarto-chunk-header {{
    background: rgba(64, 158, 255, 0.12);
    padding: 6px 12px;
    border-bottom: 1px solid rgba(64, 158, 255, 0.2);
  }}
  .chunk-badge {{
    font-family: monospace;
    font-size: 12px;
    font-weight: 600;
    color: #409EFF;
  }}
  .quarto-chunk-options {{
    padding: 6px 12px;
    background: rgba(0,0,0,0.03);
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }}
  .chunk-opt {{
    font-family: monospace;
    font-size: 11px;
    background: rgba(0,0,0,0.06);
    padding: 2px 6px;
    border-radius: 3px;
    color: #666;
  }}
  pre {{
    margin: 0;
    padding: 12px;
    overflow-x: auto;
    font-family: Consolas, 'SFMono-Regular', Menlo, monospace;
    font-size: 13.5px;
  }}
  .quarto-chunk-output {{
    background: rgba(0,0,0,0.04);
    border-top: 1px dashed rgba(64, 158, 255, 0.3);
    padding: 12px 16px;
  }}
  .output-label {{
    font-size: 12px;
    font-weight: 700;
    color: #27C93F;
    margin-bottom: 6px;
    font-family: monospace;
  }}
  .output-time {{
    font-size: 11px;
    color: #888;
    font-weight: normal;
  }}
  .output-stdout code {{
    color: {text_color};
    background: transparent;
    padding: 0;
  }}
  .output-stderr {{
    background: rgba(255, 95, 86, 0.1);
    border: 1px solid rgba(255, 95, 86, 0.3);
    border-radius: 4px;
    color: #FF5F56;
  }}
  .output-plot-container {{
    margin-top: 12px;
    text-align: center;
  }}
  .output-plot-img {{
    max-width: 100%;
    border-radius: 6px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.12);
  }}
  .math-block {{
    text-align: center;
    margin: 1.5em 0;
    padding: 14px 18px;
    background: {code_bg};
    border-radius: 8px;
    border: 1px solid rgba(64, 158, 255, 0.25);
    overflow-x: auto;
  }}
  .katex-display {{
    margin: 0.5em 0 !important;
  }}
</style>
</head>
<body>
<div class="quarto-container">
  <div class="quarto-header-card">
    <div class="quarto-badge">QUARTO DOCUMENT (.QMD)</div>
    <div class="quarto-title">{display_title}</div>
    {subtitle_html}
    <div class="quarto-meta">
      {author_html}
      {date_html}
    </div>
  </div>
  <div class="quarto-body">
    {body_html}
  </div>
</div>
<script>
  function renderAllMath() {{
    if (typeof renderMathInElement === 'function' && document.body) {{
      renderMathInElement(document.body, {{
        delimiters: [
          {{left: '$$', right: '$$', display: true}},
          {{left: '$', right: '$', display: false}},
          {{left: '\\(', right: '\\)', display: false}},
          {{left: '\\[', right: '\\]', display: true}}
        ],
        ignoredTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'],
        throwOnError: false
      }});
      return true;
    }}
    return false;
  }}

  if (document.readyState === 'loading') {{
    document.addEventListener('DOMContentLoaded', renderAllMath);
  }} else {{
    renderAllMath();
  }}
  window.addEventListener('load', renderAllMath);

  var attempts = 0;
  var timer = setInterval(function() {{
    attempts++;
    if (renderAllMath() || attempts > 60) {{
      clearInterval(timer);
    }}
  }}, 100);
</script>
</body>
</html>"#,
        title = html_escape(display_title),
        bg_color = bg_color,
        text_color = text_color,
        code_bg = code_bg,
        display_title = html_escape(display_title),
        subtitle_html = subtitle_html,
        author_html = author_html,
        date_html = date_html,
        body_html = body_html,
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QuartoFormat {
    Html,
    Pdf,
    Docx,
}

impl QuartoFormat {
    pub fn all() -> [Self; 3] {
        [Self::Html, Self::Pdf, Self::Docx]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Html => "HTML",
            Self::Pdf => "PDF",
            Self::Docx => "DOCX",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
        }
    }

    fn to_arg(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
        }
    }
}

pub struct CliExportOutcome {
    pub output: PathBuf,
    pub format: QuartoFormat,
}

/// Renders through the real Quarto pipeline, either the document on disk (so
/// `_quarto.yml`, filters and extensions apply) or a copy in `exports/` for
/// unsaved documents.
pub fn render_with_quarto_cli(
    title: &str,
    content: &str,
    doc_path: Option<&Path>,
    exports: &Path,
    format: QuartoFormat,
    open: bool,
) -> Result<CliExportOutcome, String> {
    if !has_quarto_cli() {
        return Err(
            "Quarto CLI no está instalado (https://quarto.org/docs/get-started/)".to_string(),
        );
    }

    let (source, expected) = match doc_path.filter(|p| p.is_file()) {
        Some(path) => (
            path.to_path_buf(),
            path.with_extension(format.extension()),
        ),
        None => {
            std::fs::create_dir_all(exports)
                .map_err(|e| format!("Non se puido crear {}: {}", exports.display(), e))?;
            let clean = clean_filename(title);
            let source = exports.join(format!("{}.qmd", clean));
            std::fs::write(&source, content.as_bytes())
                .map_err(|e| format!("Non se puido escribir {}: {}", source.display(), e))?;
            (
                source,
                exports.join(format!("{}.{}", clean, format.extension())),
            )
        }
    };

    let output = std::process::Command::new("quarto")
        .args(["render", &source.to_string_lossy(), "--to", format.to_arg()])
        .output()
        .map_err(|e| format!("Non se puido executar quarto: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let mut details = stderr.trim().to_string();
        if details.is_empty() {
            details = stdout.trim().to_string();
        }
        return Err(if details.is_empty() {
            format!("quarto render fallou ({})", output.status)
        } else {
            truncate_for_toast(&details, 600)
        });
    }

    // Quarto may print where it wrote the file (projects can redirect output).
    let mut rendered = if expected.exists() {
        Some(expected.clone())
    } else {
        parse_quarto_output_path(&stdout, source.parent().unwrap_or(exports))
    };
    if rendered.is_none() {
        rendered = find_rendered_file(source.parent().unwrap_or(exports), format.extension(), 2);
    }
    let Some(rendered) = rendered else {
        return Err(format!(
            "quarto render rematou pero non se atopou o {} (saída: {})",
            format.label(),
            truncate_for_toast(
                if stdout.trim().is_empty() { &stderr } else { &stdout },
                300
            )
        ));
    };

    if open {
        let _ = open_in_browser(&rendered);
    }

    Ok(CliExportOutcome {
        output: rendered,
        format,
    })
}

fn parse_quarto_output_path(stdout: &str, base: &Path) -> Option<PathBuf> {
    stdout.lines().rev().find_map(|line| {
        let line = line.trim();
        let rest = line
            .strip_prefix("Output created:")
            .or_else(|| line.strip_prefix("Output criado:"))?;
        let candidate = PathBuf::from(rest.trim());
        Some(if candidate.is_absolute() {
            candidate
        } else {
            base.join(candidate)
        })
        .filter(|p| p.exists())
    })
}

fn find_rendered_file(dir: &Path, extension: &str, depth: usize) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    let mut inspected = 0;
    while let Some((dir, level)) = stack.pop() {
        if level > depth {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            inspected += 1;
            if inspected > 2000 {
                break;
            }
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name != "_freeze" && !name.starts_with('.') {
                    stack.push((path, level + 1));
                }
                continue;
            }
            let matches = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(extension));
            if !matches {
                continue;
            }
            let modified = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                best = Some((modified, path));
            }
        }
    }
    best.map(|(_, p)| p)
}

#[derive(Clone, Debug)]
pub struct QuartoRenderOutcome {
    pub preview_path: PathBuf,
    pub used_cli: bool,
    pub cli_output: Option<PathBuf>,
    pub cli_error: Option<String>,
}

/// Renders the document twice: always to a self-contained preview (works with
/// no external tooling) and, when the Quarto CLI is installed, through the real
/// `quarto render` pipeline so citations, cross-references and projector-style
/// extensions behave like Quarto instead of a look-alike.
pub fn export_and_preview_quarto(
    title: &str,
    content: &str,
    style: TyporaStylesheet,
    kernel: Option<&PythonKernel>,
    out_dir: &Path,
    open_browser: bool,
) -> Result<QuartoRenderOutcome, String> {
    let clean = clean_filename(title);
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("Non se puido crear {}: {}", out_dir.display(), e))?;

    let preview_path = out_dir.join(format!("{}_quarto_preview.html", clean));
    let html = render_quarto_to_html(title, content, style, kernel);
    std::fs::write(&preview_path, html.as_bytes())
        .map_err(|e| format!("Non se puido escribir {}: {}", preview_path.display(), e))?;

    let mut used_cli = false;
    let mut cli_output = None;
    let mut cli_error = None;

    if has_quarto_cli() {
        let qmd_path = out_dir.join(format!("{}.qmd", clean));
        let cli_dir = out_dir.join("quarto_cli");
        let _ = std::fs::create_dir_all(&cli_dir);

        if std::fs::write(&qmd_path, content.as_bytes()).is_ok() {
            let output = std::process::Command::new("quarto")
                .args([
                    "render",
                    &qmd_path.to_string_lossy(),
                    "--to",
                    "html",
                    "--output-dir",
                    &cli_dir.to_string_lossy(),
                ])
                .output();

            match output {
                Ok(result) => {
                    let rendered = cli_dir.join(format!("{}.html", clean));
                    let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
                    let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
                    if result.status.success() && rendered.exists() {
                        used_cli = true;
                        cli_output = Some(rendered);
                    } else {
                        let mut details = String::new();
                        if !stderr.is_empty() {
                            details.push_str(&stderr);
                        }
                        if !stdout.is_empty() {
                            if !details.is_empty() {
                                details.push_str(" · ");
                            }
                            details.push_str(&stdout);
                        }
                        cli_error = Some(if details.is_empty() {
                            "quarto render fallou sen saída".to_string()
                        } else {
                            truncate_for_toast(&details, 400)
                        });
                    }
                }
                Err(e) => cli_error = Some(format!("Non se puido executar quarto: {}", e)),
            }
        }
    }

    if open_browser {
        let to_open = cli_output.as_deref().unwrap_or(&preview_path);
        let _ = open_in_browser(to_open);
    }

    Ok(QuartoRenderOutcome {
        preview_path,
        used_cli,
        cli_output,
        cli_error,
    })
}

fn truncate_for_toast(text: &str, max: usize) -> String {
    let single_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.chars().count() > max {
        let mut out: String = single_line.chars().take(max).collect();
        out.push('…');
        out
    } else {
        single_line
    }
}

static QUARTO_CLI_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<bool>>> =
    std::sync::OnceLock::new();

fn quarto_cache() -> &'static std::sync::Mutex<Option<bool>> {
    QUARTO_CLI_CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

/// Cached: the old implementation spawned `quarto --version` on every frame
/// that drew the Quarto panel or the status bar. `refresh_quarto_cli` re-checks
/// after the user installs or moves the CLI.
pub fn has_quarto_cli() -> bool {
    let mut guard = match quarto_cache().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(value) = *guard {
        return value;
    }
    let detected = detect_quarto_cli();
    *guard = Some(detected);
    detected
}

pub fn refresh_quarto_cli() -> bool {
    let detected = detect_quarto_cli();
    if let Ok(mut guard) = quarto_cache().lock() {
        *guard = Some(detected);
    }
    detected
}

fn detect_quarto_cli() -> bool {
    std::process::Command::new("quarto")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn open_in_browser(path: &Path) -> Result<(), std::io::Error> {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&abs_path).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(&abs_path).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &abs_path.to_string_lossy()])
            .spawn()?;
    }
    Ok(())
}

pub fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        out.push(CHARSET[(b0 >> 2) as usize] as char);
        out.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn format_inline_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    let mut in_bold = false;
    let mut in_italic = false;
    let mut in_math = false;
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut idx = 0;

    while idx < len {
        if chars[idx] == '`' {
            if in_code {
                out.push_str("</code>");
                in_code = false;
            } else {
                out.push_str("<code>");
                in_code = true;
            }
            idx += 1;
            continue;
        }

        if in_code {
            match chars[idx] {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                c => out.push(c),
            }
            idx += 1;
            continue;
        }

        // Inline math delimiter $
        if chars[idx] == '$' {
            if idx + 1 < len && chars[idx + 1] == '$' {
                out.push_str("$$");
                idx += 2;
                continue;
            }
            in_math = !in_math;
            out.push('$');
            idx += 1;
            continue;
        }

        // Standalone \ce{...} not already inside math mode
        if !in_math && idx + 4 <= len && &chars[idx..idx + 4] == ['\\', 'c', 'e', '{'] {
            let start = idx;
            let mut depth = 1;
            let mut end = idx + 4;
            while end < len && depth > 0 {
                if chars[end] == '{' {
                    depth += 1;
                } else if chars[end] == '}' {
                    depth -= 1;
                }
                end += 1;
            }
            let ce_str: String = chars[start..end].iter().collect();
            out.push('$');
            for c in ce_str.chars() {
                match c {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '"' => out.push_str("&quot;"),
                    ch => out.push(ch),
                }
            }
            out.push('$');
            idx = end;
            continue;
        }

        if !in_math && idx + 1 < len && chars[idx] == '*' && chars[idx + 1] == '*' {
            if in_bold {
                out.push_str("</strong>");
                in_bold = false;
            } else {
                out.push_str("<strong>");
                in_bold = true;
            }
            idx += 2;
            continue;
        }

        if !in_math && chars[idx] == '*' {
            if in_italic {
                out.push_str("</em>");
                in_italic = false;
            } else {
                out.push_str("<em>");
                in_italic = true;
            }
            idx += 1;
            continue;
        }

        match chars[idx] {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
        idx += 1;
    }

    if in_bold {
        out.push_str("</strong>");
    }
    if in_italic {
        out.push_str("</em>");
    }
    if in_code {
        out.push_str("</code>");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quarto_frontmatter_parsing() {
        let text = r#"---
title: "Documento de Proba"
author: "Investigador"
date: "2026-11-16"
format: html
---

# Contido
"#;
        let (meta, next_idx) = parse_quarto_frontmatter(text).expect("Should parse frontmatter");
        assert_eq!(meta.title.as_deref(), Some("Documento de Proba"));
        assert_eq!(meta.author.as_deref(), Some("Investigador"));
        assert_eq!(meta.date.as_deref(), Some("2026-11-16"));
        assert_eq!(meta.format, "html");
        assert!(next_idx > 5);
    }

    #[test]
    fn test_quarto_chunk_parsing() {
        let chunk_text = "#| label: fig-test\n#| echo: true\nprint('Ola Quarto')";
        let chunk = parse_quarto_chunk("{python}", chunk_text);
        assert_eq!(chunk.lang, "python");
        assert_eq!(chunk.options.len(), 2);
        assert_eq!(chunk.options[0].0, "label");
        assert_eq!(chunk.options[0].1, "fig-test");
        assert!(chunk.code.contains("print('Ola Quarto')"));
        assert!(chunk.output_preview.is_some());
    }

    #[test]
    fn test_quarto_callout_parsing() {
        assert_eq!(QuartoCalloutType::from_str("::: {.callout-note}"), Some(QuartoCalloutType::Note));
        assert_eq!(QuartoCalloutType::from_str("::: {.callout-tip}"), Some(QuartoCalloutType::Tip));
        assert_eq!(QuartoCalloutType::from_str("::: {.callout-warning}"), Some(QuartoCalloutType::Warning));
        assert_eq!(QuartoCalloutType::from_str("::: {.callout-important}"), Some(QuartoCalloutType::Important));
        assert_eq!(QuartoCalloutType::from_str("::: {.callout-caution}"), Some(QuartoCalloutType::Caution));
    }

    #[test]
    fn test_base64_encoding() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"hello world"), "aGVsbG8gd29ybGQ=");
    }

    #[test]
    fn test_python_kernel_execution() {
        let mut kernel = PythonKernel::new();
        if kernel.python_binary.is_some() {
            let res = kernel.execute_cell_blocking("test_cell", "print(100 + 42)", None);
            assert!(res.success);
            assert!(res.stdout.contains("142"));
            assert_eq!(res.execution_id, 1);
        }
    }

    #[test]
    fn kernel_keeps_state_between_cells() {
        let mut kernel = PythonKernel::new();
        if kernel.python_binary.is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("tigorra_kernel_state_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("saludo.txt"), "YAML y Python").unwrap();

        let first = kernel.execute_cell_blocking("a", "valor = 41\nimport os", Some(&dir));
        assert!(first.success, "stderr: {}", first.stderr);

        let second = kernel.execute_cell_blocking("b", "print(valor + 1)", Some(&dir));
        assert!(second.success, "stderr: {}", second.stderr);
        assert!(second.stdout.contains("42"), "stdout: {}", second.stdout);
        assert!(second.execution_id > first.execution_id);

        // Relative paths resolve against the document folder, and modules
        // imported in an earlier cell are still available.
        let third = kernel.execute_cell_blocking(
            "c",
            "print(open('saludo.txt', encoding='utf-8').read())\nprint('CWD', os.path.basename(os.getcwd()))",
            Some(&dir),
        );
        assert!(third.success, "stderr: {}", third.stderr);
        assert!(third.stdout.contains("YAML y Python"), "stdout: {}", third.stdout);
        assert!(third.stdout.contains("tigorra_kernel_state_"), "stdout: {}", third.stdout);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn kernel_captures_matplotlib_figures() {
        let mut kernel = PythonKernel::new();
        if kernel.python_binary.is_none() || !kernel.has_matplotlib {
            // Environment without matplotlib: nothing to assert here.
            return;
        }
        let res = kernel.execute_cell_blocking(
            "plot",
            "import matplotlib.pyplot as plt\nplt.figure(figsize=(2, 1))\nplt.plot([1, 2, 3])\nplt.show()",
            None,
        );
        assert!(res.success, "stderr: {}", res.stderr);
        let plot = res.plot_image_path.expect("a plot file should be captured");
        assert!(std::path::Path::new(&plot).exists());
        assert!(std::fs::metadata(&plot).unwrap().len() > 100);
        let _ = std::fs::remove_file(&plot);
    }

    #[test]
    fn kernel_can_be_interrupted_without_restarting() {
        let mut kernel = PythonKernel::new();
        if kernel.python_binary.is_none() {
            return;
        }
        kernel.request_execution("slow", "import time\ntime.sleep(30)", None);

        let mut started = false;
        for _ in 0..300 {
            kernel.poll(None);
            if kernel.is_running_cell("slow") {
                started = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !started {
            eprintln!(
                "diagnostics: binary={:?} busy={} active={:?} session_error={:?} outputs={:?}",
                kernel.python_binary,
                kernel.is_busy(),
                kernel.active.as_ref().map(|a| a.key.clone()),
                kernel.last_session_error,
                kernel.cell_outputs.keys().collect::<Vec<_>>()
            );
        }
        assert!(started, "the cell should start running");
        std::thread::sleep(Duration::from_millis(400));
        assert!(kernel.interrupt(), "SIGINT should be delivered");

        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            kernel.poll(None);
            if let Some(result) = kernel.cell_outputs.get("slow") {
                assert!(!result.success, "an interrupted cell must not report success");
                assert!(
                    result.stderr.contains("KeyboardInterrupt"),
                    "stderr: {}",
                    result.stderr
                );
                // The interpreter survives: a later cell still runs.
                let after = kernel.execute_cell_blocking("after", "print(2 + 2)", None);
                assert!(after.success, "stderr: {}", after.stderr);
                assert!(after.stdout.contains("4"));
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("the cell was not interrupted");
    }

    #[test]
    fn kernel_survives_a_failing_cell() {
        let mut kernel = PythonKernel::new();
        if kernel.python_binary.is_none() {
            return;
        }
        let bad = kernel.execute_cell_blocking("bad", "raise ValueError('boom')", None);
        assert!(!bad.success);
        assert!(bad.stderr.contains("ValueError"), "stderr: {}", bad.stderr);

        let good = kernel.execute_cell_blocking("good", "print(6 * 7)", None);
        assert!(good.success, "stderr: {}", good.stderr);
        assert!(good.stdout.contains("42"));
    }

    #[test]
    fn quarto_cli_renders_html_when_available() {
        if !has_quarto_cli() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("tigorra_cli_html_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let content = "---\ntitle: \"Cli Test\"\nformat: html\n---\n\n# Hola CLI\n\nTexto.\n";
        let outcome = match render_with_quarto_cli(
            "Cli Test",
            content,
            None,
            &dir,
            QuartoFormat::Html,
            false,
        ) {
            Ok(outcome) => outcome,
            Err(error)
                if error.contains("unable to open database")
                    || error.contains("Permission denied")
                    || error.contains("Operation not permitted") =>
            {
                // Sandboxed CI cannot write Quarto's user cache
                // (~/Library/Caches/quarto). Not an app failure.
                eprintln!("skipping CLI render test: {error}");
                return;
            }
            Err(error) => panic!("quarto CLI should render HTML: {error}"),
        };
        assert_eq!(outcome.format, QuartoFormat::Html);
        assert!(outcome.output.exists(), "{}", outcome.output.display());
        let html = std::fs::read_to_string(&outcome.output).unwrap();
        assert!(html.contains("Hola CLI"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_and_locates_quarto_outputs() {
        let dir = std::env::temp_dir().join(format!("tigorra_cli_paths_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub/report.html"), "<html></html>").unwrap();

        let parsed = parse_quarto_output_path(
            "pandoc ...\nOutput created: sub/report.html\n",
            &dir,
        )
        .expect("output line should resolve");
        assert_eq!(parsed, dir.join("sub/report.html"));

        let found = find_rendered_file(&dir, "html", 2).expect("scan finds the html");
        assert_eq!(found, dir.join("sub/report.html"));
        assert!(find_rendered_file(&dir, "pdf", 2).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quarto_export_writes_a_preview_file() {
        let dir = std::env::temp_dir().join(format!("tigorra_quarto_export_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let content = "---\ntitle: \"Informe\"\nformat: html\n---\n\n# Hola\n\nTexto de proba.\n";
        let outcome = export_and_preview_quarto(
            "Informe de Proba",
            content,
            TyporaStylesheet::GitHub,
            None,
            &dir,
            false,
        )
        .expect("export should succeed");
        assert!(outcome.preview_path.exists());
        let html = std::fs::read_to_string(&outcome.preview_path).unwrap();
        assert!(html.contains("Hola"));
        assert!(html.contains("quarto-container"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_quarto_render_html() {
        let content = r#"---
title: "Quarto Test"
author: "Galego"
---

::: {.callout-tip}
### Consello
Isto é un consello útil.
:::

```{python}
#| echo: true
print("Resultado")
```

$$E = mc^2$$
"#;
        let html = render_quarto_to_html("Quarto Test", content, TyporaStylesheet::GitHub, None);
        assert!(html.contains("QUARTO DOCUMENT (.QMD)"));
        assert!(html.contains("Quarto Test"));
        assert!(html.contains("callout-tip"));
        assert!(html.contains("Consello"));
        assert!(html.contains("Quarto"));
        assert!(html.contains("katex.min.css"));
    }

    #[test]
    fn test_quarto_multiline_math_and_mhchem() {
        let content = r#"---
title: "Quarto Ciencia"
format: html
---

# Ecuacións

A fórmula en liña é $E = mc^2$ e a reacción química é \ce{2 H2 + O2 -> 2 H2O}.

$$
y(t) = A \sin(\omega t + \phi)
$$

$$
\ce{6 CO2 + 6 H2O -> C6H12O6 + 6 O2}
$$
"#;
        let html = render_quarto_to_html("Quarto Ciencia", content, TyporaStylesheet::Newsprint, None);
        assert!(html.contains("katex.min.css"));
        assert!(html.contains("katex.min.js"));
        assert!(html.contains("contrib/mhchem.min.js"));
        assert!(html.contains("contrib/auto-render.min.js"));
        assert!(html.contains("renderAllMath"));
        assert!(html.contains("<div class=\"math-block\">$$y(t) = A \\sin(\\omega t + \\phi)$$</div>"));
        assert!(html.contains("<div class=\"math-block\">$$\\ce{6 CO2 + 6 H2O -&gt; C6H12O6 + 6 O2}$$</div>"));
        assert!(html.contains("$\\ce{2 H2 + O2 -&gt; 2 H2O}$"));
    }
}
