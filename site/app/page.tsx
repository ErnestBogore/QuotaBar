import Image from "next/image";

export default function Home() {
  return (
    <main>
      <nav className="nav shell" aria-label="Main navigation">
        <a className="brand" href="#top" aria-label="QuotaBar home">
          <Image src="/icon.png" width={34} height={34} alt="" priority />
          <span>QuotaBar</span>
        </a>
        <a className="nav-link" href="https://github.com/ErnestBogore/QuotaBar">
          GitHub <span aria-hidden="true">↗</span>
        </a>
      </nav>

      <section className="hero shell" id="top">
        <div className="hero-copy">
          <div className="eyebrow"><i /> Built for Codex on Mac</div>
          <h1>Use Codex steadily.<br /><span>Not all at once.</span></h1>
          <p className="lede">QuotaBar is a private menu-bar guardrail that helps you spread your Codex allowance across the week.</p>
          <div className="hero-actions">
            <a className="button primary" href="https://github.com/ErnestBogore/QuotaBar/releases">Download for Mac</a>
            <a className="button secondary" href="#how-it-works">See how it works</a>
          </div>
          <p className="compatibility">macOS 14+ · Apple Silicon preview · MIT licensed</p>
        </div>

        <div className="product-stage" aria-label="QuotaBar app preview">
          <div className="glow" />
          <div className="app-card">
            <div className="app-head">
              <span><Image src="/icon.png" width={25} height={25} alt="" />QuotaBar</span>
              <span className="app-controls">••</span>
            </div>
            <div className="budget-card">
              <div className="budget-label">Five-hour budget <b>?</b></div>
              <div className="budget-number"><strong>74%</strong><span>remaining</span></div>
              <div className="meter"><i /></div>
              <div className="meter-caption"><span>Resets in 3h 42m</span><span>Official</span></div>
            </div>
            <div className="week-row">
              <span>Weekly usage</span><strong>92% left</strong>
            </div>
            <div className="week-meter"><i /></div>
            <div className="app-foot"><span>How it works</span><span><i /> Pause at 0%: On</span></div>
          </div>
        </div>
      </section>

      <section className="trust shell" aria-label="QuotaBar highlights">
        <div><span>01</span><strong>Your real account meter</strong><p>Reads the usage percentages already returned for your Codex account.</p></div>
        <div><span>02</span><strong>Private by design</strong><p>Your prompts and account credentials never leave your Mac.</p></div>
        <div><span>03</span><strong>Only this Mac pauses</strong><p>Your CLI, IDE, browser and other devices keep working.</p></div>
      </section>

      <section className="section shell" id="how-it-works">
        <div className="section-intro">
          <p className="kicker">How it works</p>
          <h2>A gentle limit for one very long week.</h2>
          <p>QuotaBar recreates the old five-hour rhythm from the weekly meter OpenAI currently provides.</p>
        </div>
        <div className="steps">
          <article><span>1</span><h3>Watch the whole account</h3><p>Usage from Codex desktop, CLI, IDE, web, cloud and other devices all moves the same weekly meter.</p></article>
          <article><span>2</span><h3>Set aside a five-hour share</h3><p>QuotaBar turns part of that weekly allowance into a smaller five-hour budget that is easier to pace.</p></article>
          <article><span>3</span><h3>Pause the next Mac prompt</h3><p>At zero, the current task can finish. Only your next prompt in the Codex app on this Mac waits for reset.</p></article>
        </div>
      </section>

      <section className="split-section shell">
        <div className="split-copy">
          <p className="kicker">Built for self-control</p>
          <h2>A guardrail, not a lock.</h2>
          <p>QuotaBar is intentionally easy to understand and easy to remove. You can use one emergency 15-minute pass in each five-hour window, and Force Quit remains available.</p>
          <ul>
            <li><i />Current Codex task is never interrupted</li>
            <li><i />Unknown apps are always allowed</li>
            <li><i />No Accessibility permission or screen reading</li>
          </ul>
        </div>
        <div className="privacy-card">
          <div className="shield" aria-hidden="true">✓</div>
          <p className="kicker">Stays on your Mac</p>
          <h3>Quota data in.<br />Nothing personal out.</h3>
          <p>QuotaBar keeps its history locally. It does not upload credentials, prompts, responses, repository paths or analytics.</p>
          <a href="https://github.com/ErnestBogore/QuotaBar">Inspect the source <span aria-hidden="true">↗</span></a>
        </div>
      </section>

      <section className="section install shell" id="download">
        <div className="section-intro compact">
          <p className="kicker">Install the preview</p>
          <h2>Up and running in a minute.</h2>
        </div>
        <ol className="install-list">
          <li><span>1</span><div><strong>Download QuotaBar</strong><p>Get the newest Mac ZIP from GitHub Releases.</p></div></li>
          <li><span>2</span><div><strong>Move it to Applications</strong><p>Unzip it, then drag QuotaBar into your Applications folder.</p></div></li>
          <li><span>3</span><div><strong>Open it once</strong><p>For this unsigned preview, Control-click the app and choose Open. Future updates arrive inside QuotaBar.</p></div></li>
        </ol>
        <a className="button primary large" href="https://github.com/ErnestBogore/QuotaBar/releases">Download the Mac preview</a>
      </section>

      <section className="section faq shell">
        <div className="section-intro compact">
          <p className="kicker">Good to know</p>
          <h2>Simple answers.</h2>
        </div>
        <div className="faq-grid">
          <article><h3>Does it know whether I have Plus or Pro?</h3><p>You do not choose a plan. QuotaBar reads the percentage returned for the account already signed into Codex, so the meter naturally follows that account&apos;s allowance.</p></article>
          <article><h3>Why is the five-hour meter an estimate?</h3><p>When OpenAI returns a five-hour meter, QuotaBar uses it directly. Otherwise it reconstructs one from the official weekly movement and labels the result clearly.</p></article>
          <article><h3>Can it read my conversations?</h3><p>No. It extracts only timing, model and token counters from local Codex activity. Conversation content is never retained.</p></article>
          <article><h3>Will it block Codex everywhere?</h3><p>No. It pauses only new prompts in the Codex desktop app on this Mac. Everything else is measured but never blocked.</p></article>
        </div>
      </section>

      <footer className="footer shell">
        <a className="brand" href="#top"><Image src="/icon.png" width={28} height={28} alt="" />QuotaBar</a>
        <p>Open source under the MIT License.</p>
        <div><a href="https://github.com/ErnestBogore/QuotaBar">GitHub</a><a href="https://github.com/ErnestBogore/QuotaBar/blob/main/PRIVACY.md">Privacy</a></div>
      </footer>
    </main>
  );
}
