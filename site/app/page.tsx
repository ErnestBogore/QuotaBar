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
          <h1>QuotaBar brings back Codex’s five-hour limit.</h1>
          <p className="lede">
            One long Codex session shouldn’t burn through your weekly allowance.
            QuotaBar automatically tracks your usage and pauses new prompts in
            the Codex Mac app when your five-hour budget runs out.
          </p>
          <div className="hero-actions">
            <a className="button primary" href="https://github.com/ErnestBogore/QuotaBar/releases/latest">
              Download for Mac
            </a>
          </div>
          <p className="compatibility">macOS 14+ · Apple silicon · Free and open source</p>
        </div>

        <div className="product-stage" aria-label="QuotaBar app preview">
          <div className="glow" />
          <div className="demo-label"><i /> Five-hour window</div>
          <div className="app-card">
            <div className="app-head">
              <span><Image src="/icon.png" width={25} height={25} alt="" />QuotaBar</span>
              <span className="app-controls">••</span>
            </div>
            <div className="budget-card">
              <div className="budget-label">Five-hour budget <b>?</b></div>
              <div className="budget-number animated-budget">
                <strong aria-label="Five-hour budget counts down from 74 percent to zero">
                  <span className="value value-74">74%</span>
                  <span className="value value-28">28%</span>
                  <span className="value value-8">8%</span>
                  <span className="value value-0">0%</span>
                </strong>
                <span>remaining</span>
              </div>
              <div className="meter"><i className="demo-meter-fill" /></div>
              <div className="meter-caption"><span>Resets in 3h 42m</span><span className="meter-state"><b className="state-ready">On track</b><b className="state-warning">Almost out</b><b className="state-paused">Limit reached</b></span></div>
            </div>
            <div className="week-row">
              <span>Weekly usage</span><strong>92% left</strong>
            </div>
            <div className="week-meter"><i className="demo-week-fill" /></div>
            <div className="app-foot"><span>Weekly meter keeps tracking</span><span className="gate-state"><i /><b className="gate-ready">Mac prompts ready</b><b className="gate-warning">Warning</b><b className="gate-paused">Mac prompts paused</b></span></div>
          </div>

          <div className="prompt-demo" aria-label="A new Codex prompt is paused when the limit is reached">
            <span className="prompt-kicker">New Codex prompt</span>
            <p>“Make one more change to the app.”</p>
            <div className="prompt-result">
              <i />
              <div>
                <strong><span className="prompt-ready">Ready to send</span><span className="prompt-paused">Paused by QuotaBar</span></strong>
                <small><span className="prompt-ready">Budget available</span><span className="prompt-paused">Try again after reset</span></small>
              </div>
            </div>
          </div>
        </div>
      </section>
    </main>
  );
}
