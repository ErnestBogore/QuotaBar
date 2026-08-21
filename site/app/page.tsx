import Image from "next/image";

export const dynamic = "force-dynamic";

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
            <a
              className="button primary"
              href="https://github.com/ErnestBogore/QuotaBar/releases/latest/download/QuotaBar.dmg"
              data-site-version="9"
            >
              Download for Mac
            </a>
          </div>
          <p className="compatibility">macOS 14+ · Free and open source</p>
        </div>

        <div className="product-stage" aria-label="QuotaBar app preview">
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
        </div>
      </section>
    </main>
  );
}
