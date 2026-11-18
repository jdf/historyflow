# History Flow (Viégas, Wattenberg, Dave; IBM Research, 2003–2005): research notes for a reimplementation

Research date: 2026-11-18.

Tagging convention:

- **[stated]** means a source says it explicitly. The URL follows the tag.
- **[observed]** means I read it off a published screenshot or figure. The
  figure is named, and any reading of it is still an interpretation.
- **[inferred]** means my own reasoning. Treat it as a design suggestion, not a
  fact about the original.
- **[probe]** means I ran a live API request on 2026-11-18 and report the
  result.

Short names for the main sources (full URLs are in the sources list):

- **CHI04**: Viégas, Wattenberg, Dave, _Studying Cooperation and Conflict
  between Authors with history flow Visualizations_, CHI 2004. Full PDF read:
  https://politics.media.mit.edu/papers/history_flowviegas.pdf
- **IBM-how**, **IBM-results**, **IBM-gallery**: the IBM Research project pages,
  read through the Wayback Machine. Base URL:
  http://www.research.ibm.com/visual/projects/history_flow/ (pages
  `explanation.htm`, `results.htm`, `gallery.htm`)
- **AW**: the alphaWorks release page, March 2005, updated November 2005, read
  through Wayback: http://www.alphaworks.ibm.com/tech/historyflow (plus
  `/requirements` and `/download`)
- **THESIS**: Viégas, PhD thesis, MIT 2005, §3.3:
  https://smg.media.mit.edu/papers/Viegas/viegas_thesis.pdf
- **TBYT**: Viégas, Wattenberg, Kriss, van Ham, _Talk Before You Type:
  Coordination in Wikipedia_, HICSS 2007:
  http://hint.fm/papers/wikipedia_coordination_final.pdf

---

## 1. Visual encoding

### Axes

- **X axis is the revision sequence.** By default versions are equally spaced.
  An alternative mode spaces them in proportion to elapsed time. [stated] CHI04
  p.577; TBYT §3.2: "plotting revision sequence on the x-axis and using the
  y-axis to show how the contributions of different authors are added, deleted,
  and rearranged over time."
- **Y axis is position within the document.** Top is the start of the text and
  down is later text. [stated] CHI04 Fig. 2 narrative: Suzanne's appended text
  appears "at the bottom of Mary's original line". The "age mode" thumbnail text
  on IBM-how reads "document position is on the y-axis".
- **Versions are top-aligned at y = 0.** Total length grows downward, so the
  black area below the content traces page length over time. [observed] in every
  figure (CHI04 Figs. 3–7; IBM-results IBM and Abortion images). CHI04 p.580
  says: "A history flow visualization is, in effect, a fancy graph of how the
  length of a page varies over time".
- **Time labels sit along the bottom edge.** Fig. 3 shows "November 2001 … 16
  Nov 19:45 2002 … June 2003", and the IBM figure shows "December 2001 … March
  2002". The selected version's timestamp, to the minute, is shown under its
  line. [observed] in CHI04 Fig. 3 and the IBM-results images. [stated] CHI04
  p.577: "the date of the selected version (down to the nearest minute) is
  displayed at the bottom".

### Revision lines

- **Each version is a vertical "revision line"** whose length is proportional to
  the length of its text. Sections of the line are colored by "who originally
  authored them". [stated] CHI04 p.577. AW says "each document as a vertical
  line whose length corresponds to the length of the document."
- **Unit of length.** The sources only say "length of its text". The tokens are
  sentences (§4), so each sentence segment's height is presumably proportional
  to its character count, not 1 unit per sentence. Otherwise a one-word sentence
  and a long paragraph-sentence would have equal height, which the screenshots
  do not show. [inferred] Nothing states characters versus words. CHI04 measures
  page size in characters elsewhere ("decreasing it by more than 50 characters",
  p.581), and the size charts are in kilobytes (Fig. 8).
- **Vertical scale.** The scale appears to be one global pixels-per-character
  factor, set so the largest version fits the plot height. [inferred] from short
  early versions occupying a small fraction of the height in Figs. 3, 4 and 7.
- **Revision lines also appear as thin, faint gray hairlines spanning the full
  plot height**, including the empty black area below the text. They are visible
  as regularly spaced vertical rules in Figs. 3, 4 and 7 and in the IBM images.
  [observed]
- **Small colored ticks at the top of each revision line.** In the IBM 8-version
  image these are green, yellow, purple and white, matching the author list.
  They probably mark the author of that revision. [observed and inferred]

### Connections (ribbons)

- **The key step is linking unchanged text.** "The key step in a history flow
  diagram is to visually link sections of text that have been kept the same
  between consecutive versions. To do so, we draw shaded connections between
  corresponding segments on adjacent revision lines." [stated] CHI04 p.577
- **The ribbons are parallelograms.** "the matches are represented onscreen by a
  parallelogram connecting the appropriate sections of the document segments.
  Both segments and parallelograms are colored to indicate authorship." [stated]
  AW
- **A ribbon is a straight-edged quadrilateral, not a curve.** Its left edge is
  the segment's span on line _i_, its right edge is the span on line _i+1_, and
  it is filled with the author color. [inferred] from the word "parallelogram"
  and the straight diagonal edges in every figure. Because matched text has the
  same length in both versions, both edges have equal height, so the shape
  really is a parallelogram (sheared vertically).
- **Ribbon color versus line color.** The explanatory diagram on IBM-how (images
  `1.gif`…`5.gif`) shows saturated revision-line segments (orange, blue, green)
  and the ribbons as darker, desaturated versions of the same hue. [observed] In
  the real application screenshots ribbons and lines are hard to tell apart; the
  lines read as slightly brighter columns. [observed]
- **Moves produce crossing ribbons.** "people tend to delete and insert text
  more frequently than moving text… we see many more 'gaps' in the visualization
  than the type of crossing lines in Fig. 5." [stated] CHI04 p.581. IBM-results
  "ibm2date" shows several green bands crossing as a block moves up. [observed]
  So the matcher has to report moves, and ribbons are allowed to cross.

### Insertions and deletions

- **Unmatched text gets no ribbon.** "Pieces of text that do not have
  correspondence in the next (or previous) version are not connected and the
  user sees a resulting gap in the visualization, clearly highlighting deletions
  and insertions." [stated] CHI04 p.577
- **Insertions and deletions are drawn as gaps, not tapers.**
  - An insertion appears as a black triangular or trapezoidal gap to the left of
    the new segment. The neighboring ribbons diverge to make room.
  - A deletion appears as a gap to the right of the deleted segment.
  - [observed] in IBM-how `5.gif` (the space between version 2's ribbons and
    Suzanne's new blue segment) and throughout the screenshots. There is no
    tapering wedge from zero height. The new segment simply appears on its
    revision line with empty background before it.
- **Background is black** in the application. [observed] in CHI04 Figs. 3–7.
  CHI04 p.578 also says "the abrupt black gutters represent instances of mass
  deletions." The explanatory diagrams (CHI04 Fig. 2, IBM-how) use a gray or
  dotted background, but those are illustrations. [observed]

### Mass deletion and repair (the "black gutter")

- **What a vandalised version looks like.** The vandalised version's line is
  nearly empty. The preceding version's ribbons end in a gap, and the next
  version re-inserts the text, also through a gap. The result is a full-height
  black vertical stripe in equal-spacing mode. It disappears in date-spacing
  mode because the repair arrives within minutes. [stated] CHI04 p.578 (Figs. 4,
  5); IBM-results "Vandalism".
- **Restored text keeps its original authorship colors.** In the enlarged CHI04
  Fig. 4 (Abortion), the colored stripes on the right of each black gutter
  repeat the same colors in the same order as on the left. [observed] So the
  original did not assign restored sentences to the reverter. See §4 for what
  this implies about its attribution.

---

## 2. Modes and coloring

### The four color modes

**[stated]** IBM-how and AW (identical wording): "history flow has four main
visualization modes":

1. **Community view.** "this is the default mode and it shows all contributions
   from different authors, color-coding the text to indicate the author of each
   sentence."
2. **Individual author view.** "highlights the contributions of a single author
   and it depicts the persistence of these contributions over time."
3. **Recent Changes View** (the button is labeled "text changes"). "highlights
   the new content in each version of the Wiki page independent of authorship.
   This view allow us to see what portions of the text have been edited the most
   over time."
4. **Age View** (the button is labeled "text age"). "this view has no colors
   representing authorship; instead, the focus is on the persistence of
   different contributions. A gray scale gradient goes from white (brand new
   contribution) to dark gray (very old contribution)."

### Toolbar and spacing

- **Color buttons** in the CHI04 Fig. 3 toolbar, each with a small icon:
  **group**, **individual**, **text changes**, **text age**, **links**.
  [observed]
- **Spacing control:** radio buttons labeled **date** and **version**.
  [observed] in Fig. 3.
- **"links" mode is never described in any source I found.** It probably
  highlights wiki-link markup, since the text panel renders links underlined and
  colored. [inferred, unverified]

### How each mode looks

- **Age inside community view.** Age is also applied as brightness on top of
  author hue: "Text that persists over time is darkened to indicate its age, so
  at the right side of the diagram Peter Winnberg's contributions have changed
  from bright green to dark green." The caption reads "Hues indicate authorship;
  brightness indicates age of text, with brighter colors being more recent."
  [stated] IBM-results.
  - So the community view is hue = author and value/brightness = age.
  - I found no stated decay rate. Whether age is counted in versions or
    wall-clock time is not stated. [inferred]
- **Individual author view.** The selected author's text is drawn bright (light
  yellow in the Islam example: "the highlighted portions of the visualization
  (light yellow) represent text contributed by a single author"). Other text is
  dimmed but keeps a muted hue. [stated] IBM-results "Persistence"; [observed]
  `small_individual_author.gif` and `individual_thumbnails.gif`.
- **Recent changes view.** Text that is new in a version is drawn in a bright
  color (the thumbnail shows green). Everything else is drawn in muted author
  colors. [observed] `recent_changes_thumbnails.gif`. Whether "new" fades over
  the next few versions or applies only to the version that introduced it is not
  stated. [inferred]

### Spacing by revision and by date

- **By revision:** equal x spacing.
- **By date:** "we let the space between successive revision lines be
  proportional to the time between the revision dates… de-emphasizes revisions
  that come in rapid succession." [stated] CHI04 p.577
- In date mode, close revisions collapse onto nearly the same x, as in Fig. 2C
  ("2 3"). [observed]

### Author colors and anonymous authors

- Each contributor is "assigned a different color". [stated] CHI04 p.577
- Anonymous authors are white or gray: "Several anonymous authors also made
  contributions; their insertions are shown in shades of gray." "White and gray
  represent the contributions of anonymous authors." [stated] IBM-results. CHI04
  p.580 says pages written largely by anonymous users "show up as diagrams
  mostly in shades of gray".
- Each anonymous IP probably gets its own gray level. [inferred] from the
  visible variety of grays and whites.
- I found no statement of the palette or of how hues are assigned. The
  screenshots show a fairly saturated, light palette on black: greens, salmon,
  peach, lavender, yellow, cyan. [observed] Assignment by first-appearance order
  or by hashing the name are both consistent with the images. [inferred]
- **Bot or script authors are ordinary named authors.** "Conversion script" has
  its own yellow color in the IBM example. [stated] IBM-results: "three named
  authors (listed at left), including a script which changed some formatting".

### Selection highlighting

- When a revision line is selected:
  - "The author's comment is displayed at the top of the revision line"
  - "the date of the selected version (down to the nearest minute) is displayed
    at the bottom"
  - "all other versions by that author are highlighted."
  - [stated] CHI04 p.577
- In Fig. 3 the comment appears as `K.lee "fixing trustworthy computing link"`
  above the plot, and small white dots below the axis appear to mark that
  author's other versions. [observed]

---

## 3. Interaction and window layout

### Layout

- **Three columns.** The source describes the layout as "on the right we see the
  contents of the page, on the left we see all the authors who have contributed
  to this page; the center panel shows the visualization." [stated] CHI04 Fig. 3
  caption. CHI04 p.577: "The bulk of the screen is devoted to the history flow
  visualization itself. Above it are buttons that let the user change the color
  scheme… To the side of it is a text panel closely linked with the
  visualization."
- **Header row** [observed] in Fig. 3:
  - the page title (`"Microsoft" Wikipedia`) at the top left
  - **COLOR** with the five icon buttons
  - **SPACING** with the date and version radio buttons
  - a dark gray header against a black plot
- **Left panel.** Two tab buttons, **authors** and **posts**. The authors list
  shows a small color swatch, the author name and a count. The count is probably
  the number of revisions by that author (Hannes Hirzel 4, The Anome 14, Stephen
  Gilbert 10). [observed] What "posts" lists (probably revisions with comments)
  is not documented. [inferred]
- **Right panel.** Shows the article text at the selected version and position.
  The rendered text uses author colors for the text itself (orange, white, gray
  runs), so the text panel matches the plot's coloring. Wiki links are
  underlined, and section headings ("Hardware", "History of Microsoft") are
  styled. [observed] in Fig. 3.
- **Menus.** The alphaWorks-era build has a menu bar. The Islam screenshot shows
  something like "File … Color" in the title bar region. [observed, low
  resolution; unverified]

### Crosshairs and linking

- **Crosshairs drive the text panel.** "if the user moves a set of crosshairs to
  a location on the visualization, the text view shows the text for the
  corresponding version and position within that version. Conversely, scrolling
  the text view will move the marker on the visualization. This tight linking of
  overview and detail was critical for effective analysis." [stated] CHI04 p.577
- **The crosshair is a vertical white line** at the selected version plus a
  **horizontal orange line** across the plot at the selected position.
  [observed] in Fig. 3. A user of the alphaWorks release described it as "slice
  the graph (the vertical line) and see the color coded text at each point"
  (Slashdot comment).
- **Hover versus click.** No source says whether moving the crosshair needs a
  click and drag or just a hover. [unverified]

### Zoom and scroll

- **The 2003 to 2004 tool fit all versions on screen.**
- **Later rewrite.** For the 2005 dump the authors rewrote rendering: "The most
  important was the creation of a user interface that allowed smooth zooming and
  panning, since many articles had more revisions than could fit on the screen.
  This in turn required rewriting the rendering layer of the application to use
  OpenGL rather than the standard Java2D package." [stated] TBYT §3.2.

### Platform and data plug-ins

- **Platform.** "Our application is written in Java 1.4 and runs as a standalone
  program." [stated] IBM-how
- **Distribution.** Installers for Windows, Linux and Mac OS X. [stated] AW
  `/download`
- **Data plug-ins.** "a plug-in for retrieving the history of a given page from
  any MoinMoin wiki". [stated] AW. Slashdot also mentions MediaWiki. The
  alphaWorks forum had a thread titled "new version mediawikisupport" (Nov
  2005). I could not open the thread.

---

## 4. Tokenization and diff in the original

### Unit: "sentences"

- "we chose a simple technique that works by matching up tokens [7] — in our
  program 'sentences' are defined as pieces of text delimited by periods or html
  tags — which gives decent results with sufficient efficiency. One problem with
  this approach is that tiny changes, such as the addition of a single comma,
  will show up as a change to an entire sentence." [stated] CHI04 p.578. THESIS
  §3.3.5 is identical.
- AW words it as "applies a standard 'diff' algorithm to successive versions of
  a document, using periods and angle brackets to indicate changes." [stated] AW
- **Not specified anywhere:**
  - whether the delimiter belongs to the preceding token
  - how whitespace or newlines are treated
  - whether wiki markup such as `[[ ]]`, `''` or `==` was stripped
  - whether newlines alone split tokens
  - [unverified]
- Two consequences of a naive split on `.`, `<` and `>`: abbreviations ("U.S.",
  "e.g.") split sentences, and an HTML tag becomes its own token. [inferred]
  That is harmless for a faithful reimplementation.
- **Recommended faithful tokenizer** [inferred]:
  - Split after every `.`, before every `<`, and after every `>`.
  - Keep delimiters attached to the preceding token.
  - Compare tokens exactly, perhaps after trimming whitespace.

### Matching algorithm: Heckel 1978

- **The paper cites Heckel.** CHI04 cites "[7] Heckel, Paul. A Technique for
  Isolating Differences Between Files. CACM 21(4), pp. 264–268, April 1978."
  IBM-how says: "we've chosen a simple technique from the 70s. See Heckel,
  Communications of the ACM, April 1978". [stated]
- **How Heckel works** [stated] from the comments and code of `hdiff`, a 1986 C
  implementation of Heckel
  (https://www.tuhs.org/Usenet/mod.sources/1986-February/000410.html):
  - Pass 1 reads the new file. Each line is hashed into a symbol table entry
    that counts its occurrences in the new file (NC).
  - Pass 2 reads the old file, counting occurrences in the old file (OC) and
    recording the old line number.
  - Pass 3 "match up lines which occur only once" in both files (OC = NC = 1)
    and links them. It also links virtual begin and end lines.
  - Pass 4 handles a line _i_ in the new file that is linked to old line _j_. If
    new line _i+1_ and old line _j+1_ are unlinked but have the same symbol, it
    links them too. This runs ascending ("apply rule 2 working toward end").
  - Pass 5 does the same descending.
  - Pass 6 emits the differences.
  - Linked pairs that are not in ascending order are **moved blocks**. The hdiff
    author notes that Heckel's algorithm "detects moved blocks whereas UNIX diff
    reports it as delete/add blocks."
- **Properties relevant here** [inferred]; these are standard properties of the
  algorithm, not stated in the history flow sources:
  - It runs in linear time and space.
  - It detects moves and transpositions natively. Unique tokens anchor anywhere,
    which is why ribbons can cross.
  - It fails when no token is unique. For example, repeated boilerplate
    sentences that are not adjacent to an anchor stay unmatched.
  - It can produce spurious anchors when a unique token coincidentally
    reappears.
  - With sentence tokens, most sentences in a Wikipedia article are unique, so
    Heckel works well. That is a plausible reason they chose it.
  - It only compares two versions. It has no memory of deleted text.

### Moves

- Moves are shown as crossing ribbons and were rare in practice. [stated] CHI04
  p.581
- Heckel's unique-anchor linking yields these crossings automatically.
  [inferred]

### Reverts, mass deletions and edit wars

- **Mass deletions:**
  - the deleted version's line is very short
  - nothing connects to the restored version, because v(i-1) and v(i+1) are not
    adjacent
  - this gives a full-height black gutter in equal spacing
  - [stated and observed] CHI04 Fig. 4
- **Edit wars:**
  - "A second pattern revealed by our visualizations is a zigzag arrangement
    that lasts for a few versions before dying out… edit wars… where two people
    or groups alternate between versions of the page." [stated] CHI04 p.579
  - Mechanism [inferred and observed]: a paragraph is alternately inserted and
    removed. Every ribbon below it shifts down, then up, then down, which
    produces parallel diagonal bands in a zigzag (CHI04 Fig. 6, Chocolate). The
    bands above the disputed paragraph stay horizontal.

### Carrying authorship forward

- Revision-line segments are "colored according to who originally authored
  them". [stated] CHI04 p.577
- The explicit wording only covers consecutive versions. Heckel compares only
  version _i_ with _i+1_.
- **But the Abortion figure shows restored text in its original colors after
  each gutter.** [observed] in CHI04 Fig. 4, enlarged.
- That is impossible if authorship came only from the immediately preceding
  version, because the vandal's version contains almost nothing. [inferred]
- **The most likely mechanism** [inferred]: a global table keyed by sentence
  text, recording the author (and time) of a sentence's first appearance
  anywhere in the history. Any later occurrence of an identical sentence
  inherits that origin, whether or not it was in the previous version. With
  exact-match sentence tokens this is a one-line hash-map lookup. It produces
  exactly the "restored text keeps its color" behavior, while the ribbons (from
  pairwise Heckel) still show a gap.
- **Unknown:** whether a revert by an author who first typed a sentence
  differently also inherits the origin, and how identical duplicate sentences
  inside one version were resolved. [unverified]
- **Recommendation:** implement attribution through a history-wide token-origin
  table, separately from the pairwise ribbon matching (§7). [inferred]

---

## 5. Named patterns and example articles (acceptance tests)

All from CHI04 or THESIS unless noted. The data were the Wikipedia dumps of May
2003 (statistics) and August 2003 (visualizations; Microsoft had "198 versions
comprising 6.2 MB of text" as of August 2003). [stated]

| Pattern                                                            | What it looks like                                                                | Example articles (source)                                                                                                                                                                                                                                                                                                            |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Vandalism and repair: mass deletion**                            | One-version full-height black gutters in equal spacing, invisible in date spacing | **Abortion**, Figs. 4 and 5 (CHI04 p.578; IBM-results). THESIS: "Each mass deletion took only a minute or so to be fixed!"                                                                                                                                                                                                           |
| **Offensive copy, phony copy, phony redirect, idiosyncratic copy** | Single-version insertions: a lonely segment with no ribbons on either side        | **Chemistry** (full Windows 98 readme pasted); **Israel** redirected to "feces"; **Islam** (long Koran passages); **Cat** (Unix `cat` diatribe) (CHI04 p.579). THESIS Fig. 30, **Cat**: "this insertion of new material looks like a lonely white line… not linked to any of the subsequent versions"; it was moved to "Cat (Unix)". |
| **Edit war / negotiation**                                         | Zigzag lasting a few versions                                                     | **Chocolate**, Fig. 6: two users fought over "coulage"; "12 consecutive versions of reverting back and forth" (CHI04 p.579). Visible only in equal spacing (THESIS Fig. 32).                                                                                                                                                         |
| **Anonymous versus named authorship**                              | Mostly gray/white diagrams versus mostly colored ones                             | Mostly anonymous: **Microsoft** (Fig. 3), Sex, Music, Libertarianism, Creation, Computer. Few anonymous: **Mythology, Evolution, Design, Brazil** (Fig. 7) (CHI04 p.580). IBM-results contrasts Evolution with Microsoft.                                                                                                            |
| **Prolific anonymous contributor**                                 | One gray level persisting                                                         | **Capitalism**: an anonymous user edited 55 times between 22 Nov 2002 and 26 Jun 2003 (CHI04 p.580)                                                                                                                                                                                                                                  |
| **Growth: steady versus bursty**                                   | Black area shrinking steadily, or a sudden jump                                   | **Microsoft**: near-constant growth. **Abortion**: growth and shrinkage; the biggest shrink came when material moved to "abortion in Ireland". **Brazil**: "a sudden growth of over 100% of its size" (CHI04 p.580; IBM-results)                                                                                                     |
| **First-mover advantage / persistence**                            | The first author's color survives longest, darkening with age                     | General observation (CHI04 p.580–581). **IBM**: Peter Winnberg's green persists (IBM-results). **Islam**: one author's highlighted text persists (IBM-results "Persistence")                                                                                                                                                         |
| **Insert/delete much more common than moves**                      | Many gaps, few crossings                                                          | CHI04 p.581                                                                                                                                                                                                                                                                                                                          |
| **Rapid assimilation of news**                                     | Bursty growth                                                                     | **Iraq** tripled in size after the March 2003 invasion (CHI04 p.581); gallery has Iraq by time and by version (IBM-gallery)                                                                                                                                                                                                          |
| **Talk-page archiving cycles** (later work)                        | Sawtooth: growth, then mass removal                                               | **Talk:God** (TBYT Fig. 7)                                                                                                                                                                                                                                                                                                           |

**Gallery articles** (IBM-gallery): Islam (×2 plus text-changes mode), Evolution
(×2), Iraq (time and versions), Love (×2), Calculus (versions and date),
Abortion (versions and date), Capitalism (versions and text-changes).

**Statistics usable as sanity checks** (CHI04 Table 1, May 2003):

- mass delete defined as ≥90% smaller than the previous maximum size, not a
  redirect, not by an administrator
- median survival 2.8 minutes (n = 3,574)
- "MD obscene" median 1.7 minutes (n = 47)
- all content median 90.4 minutes
- TBYT recomputed for 2005: 2.9 min / 2 min

**[probe] What the live MediaWiki API returns today** for these titles up to
2003-08-31:

| Article    | Revisions |
| ---------- | --------- |
| Chocolate  | 53        |
| Abortion   | 102       |
| Microsoft  | 250       |
| IBM        | 43        |
| Brazil     | 74        |
| Evolution  | 223       |
| Islam      | 307       |
| Cat        | 65        |
| Capitalism | 187       |

- **Chocolate:** the coulage dispute is visible in the comments ("reverting;
  coulage is not a 'Boyer invention'", 2003-07-27 and 2003-07-28).
- **Caution 1:** Microsoft has 250 revisions to August 2003 here, against the
  paper's 198.
- **Caution 2:** IBM's first live revision is an anonymous edit on 2001-10-16,
  then "Ap", then Peter Winnberg on 2001-12-23. IBM-results says Peter Winnberg
  was the initial author of the first eight versions shown.
- **Interpretation** [inferred]: early-2001 UseModWiki history was imported into
  the live database after 2003, so today's API history is not identical to the
  authors' dataset. Acceptance tests should compare qualitative patterns, not
  exact version counts. An option to skip pre-2002 imported revisions would help
  reproduce the paper's figures.

---

## 6. Scale, performance, data acquisition

- **Scale:**
  - Microsoft, 198 versions / 6.2 MB (CHI04 p.576)
  - "more than 70 different Wikipedia page histories" examined (CHI04 p.578)
  - the requirements page asks for "At least 500 MB of RAM (the larger document
    histories are quite memory-intensive)" [stated] AW `/requirements`
  - the 2005 data needed zooming and panning and an OpenGL renderer (TBYT)
  - TBYT Fig. 4 shows Chocolate by October 2005 compressed so far that the 2003
    edit war became indistinguishable
- **Performance techniques stated:** only the choice of a "simple", linear-time
  sentence-level Heckel ("decent results with sufficient efficiency"), and the
  OpenGL rewrite. [stated] No other optimizations are documented. [unverified]
- **Data source:**
  - full Wikipedia database dumps from download.wikipedia.org, loaded into MySQL
    for statistics
  - acknowledgement to "Jimmy Wales who were gracious enough to provide us with
    data"
  - [stated] CHI04 pp.578, 582; IBM-how links to download.wikipedia.org
  - the released tool fetched page histories through wiki plug-ins (MoinMoin,
    later MediaWiki) [stated] AW, Slashdot

---

## 7. Diff and attribution algorithm survey and recommendation

### What the system actually needs

1. **Ribbons:** for each pair of consecutive versions, a set of matched spans
   (old range ↔ new range), crossings allowed.
2. **Attribution:** for each token in each version, an origin (revision, author,
   time) that survives delete-then-restore.
3. Scale: hundreds to thousands of versions, interactive in WASM.

### Candidates

**Heckel 1978** (CACM 21(4):264–268)

- Unique-token anchors, extended by neighbor propagation in forward and backward
  passes. [stated] hdiff.
- O(n) time and space. Detects moves natively.
- Weak on repeated tokens: no anchor means no match, so repeated blank lines,
  short sentences like "See also." or identical list items fall out unless
  adjacent to an anchor. It can also produce a bad anchor from a coincidental
  unique match. Pairwise only, so no reintroduction. [inferred]
- No published accuracy numbers for authorship.
- Crate: `heckel-diff` 1.0.0. It says "Implementation Paul Heckel's algorithm";
  its API is `diff`, `new_version`, `old_version` returning `Change`s. It was
  published 2026-03 with 27 downloads, so it is immature; I did not verify
  whether `Change` represents moves. [stated] https://docs.rs/heckel-diff,
  crates.io.

**Myers 1986 O(ND)** (Algorithmica 1:251–266)

- Computes a minimal edit script (LCS). Fast when versions are similar (D
  small).
- **Cannot represent moves.** A moved block becomes delete plus insert, so
  ribbons can never cross.
- Pairwise only. Can produce unintuitive alignments on repeated tokens; patience
  and histogram address that.
- Complexity is standard from the literature; I did not fetch the paper.
  [inferred]

**Patience diff** (Bram Cohen)

- Anchors on tokens unique in both sides, takes the longest increasing
  subsequence of the anchors, then recurses between them.
- More readable diffs than Myers on code.
- Still order-preserving: the LIS discards out-of-order anchors, so **no
  moves**. Pairwise. [inferred; standard description]

**Histogram diff** (JGit/git)

- An extension of patience that uses low-occurrence (not just unique) tokens as
  anchors.
- Nugroho et al. (EMSE 2020) found Histogram gave a better diff than Myers in
  62.6% of files with code changes, against 16.9% the other way. Algorithm
  choice changed results in 1.7–8.2% of commits. [stated]
  https://arxiv.org/abs/1902.02467
- Order-preserving, so no moves. Pairwise.

**Tichy 1984 block-move** (ACM TOCS 2(4))

- Greedy covering of the new string by longest common substrings of the old one.
  Copies and moves are first-class.
- Near-linear with suffix trees.
- de Alfaro & Shavlovsky note that "Tichy-inspired algorithms were found to be
  highly efficient, and as precise as any alternative, for the problem of
  comparing two revisions." [stated]
  https://archives.iw3c2.org/www2013/proceedings/p343.pdf §1
- Pairwise in its pure form.

**WikiTrust** (Adler & de Alfaro, WWW 2007, "A content-driven reputation system
for the Wikipedia")

- Word tokens.
- Keeps, per page, a "live chunk" (current text, each word labeled with its
  origin version) and "dead chunks" (previously deleted text).
- Matching is a greedy algorithm that takes highest _match quality_ first:
  - for the live chunk: l/min(m_{i+1}, m′) − 0.3·|k′/m′ − k_{i+1}/m_{i+1}|, so
    long matches at the same relative position win
  - for dead chunks: 0 if l < 4, else l/min(m_{i+1}, m′) − 0.4
  - the same text may be matched multiple times, to resist duplication attacks
  - coefficients were tuned against human judgments on Italian Wikipedia
  - [stated]
    https://web.archive.org/web/2008id_/http://www2007.org/papers/paper692.pdf
    §3.1
- Handles moves (greedy LCS-style) and reintroduction (dead chunks).
- Accuracy: Flöck's evaluation found 48.4% precision for WikiTrust attributions.
  [stated] Flöck dissertation (ch. 4 related work, citing their earlier
  tree-model study):
  https://publikationen.bibliothek.kit.edu/1000058165/3879557. TokTrack
  summarizes it as "only around 50%". [stated] https://arxiv.org/pdf/1703.08244
- de Alfaro & Shavlovsky note that WikiTrust's attribution "depends on the
  reputation computation".

**de Alfaro & Shavlovsky, "Attributing authorship of revisioned content"** (WWW
2013, algorithm A3), "earliest plausible attribution". [stated] p343.pdf:

- A token's origin is the earliest revision compatible with any _interesting_
  match. A match is interesting if its rarity γ ≥ Δ, with γ = sequence length in
  their implementation and threshold 4 words by default.
- The match may be against _any_ previous revision.
- Implemented with a trie of all "minimally interesting" n-grams (n = Δ) seen in
  history, each labeled with its minimal origin labels.
- Per-revision update is O(len(ρn)) time; the summary is O(total change).
- Content aging prunes trie nodes unseen for ΔN revisions and ΔT days. With ΔN =
  100 / ΔT = 90 days this changed attribution by 2.1% and cut the trie to 55%.
- Tokens are whitespace-separated words.
- Attribution differs by over 75% from previous-revision-only attribution (A0),
  "due chiefly to deletion-reinsertion attacks and other vandalism". It differs
  only a few percent from a Tichy-based global variant (A2) for Δ ≥ 3.
- Pros: principled, robust to delete/restore and partial restores, and handles
  moves implicitly, since order does not matter for n-gram matches.
- Cons: it produces _attribution_, not an alignment, so it does not directly
  give ribbons. Common n-grams (rarity = length) can misattribute.
- Precision on Flöck's gold standard: 0.77 (A3 with markup filter off), against
  WikiWho's 0.95. [stated] Flöck dissertation Table 2.

**WikiWho** (Flöck & Acosta, WWW 2014). [stated] source:
https://github.com/wikiwho/WikiWho `wikiwho.py`/`utils.py`, and the Flöck
dissertation.

- A hierarchy of paragraph → sentence → token.
- **Paragraphs:**
  - split on `\n\n`, with tables forced into their own paragraphs
  - MD5-hashed
  - looked up first in the previous revision, then in a hash table of _all_
    paragraphs ever seen
  - an unmatched paragraph is split into sentences
- **Sentences:**
  - split after `xyz. ` (regex `[^\s.=]{3}\. `), `; `, `? `, `! `, `: `, tab and
    newline
  - `<!-- -->` comments, `<ref …/ref>` and URLs are made into their own
    sentences
  - hashed and looked up the same way, previous revision then all history
- **Tokens:** remaining unmatched sentences are tokenized (whitespace plus
  wiki-markup characters as both delimiters and tokens) and diffed against the
  unmatched words of the previous revision with Python `difflib.Differ`.
- **Per-token tracking:** origin revision plus lists of out-revisions (deleted)
  and in-revisions (reinserted).
- **Vandalism filters:**
  - a revision that shrinks by 40% or more from a previous size above 1000
    characters is skipped as spam
  - its hash is remembered so identical reposts are skipped too
  - a token-density limit (average word repetition > 20) also flags spam
- **Accuracy:** precision **0.95** on a 240-token, 40-article crowd-validated
  gold standard (218 tokens usable), against 0.77 for A3. The advantage is
  largest for articles with more than 5k revisions. Earlier tree-model WikiWho
  scored 59.2% against WikiTrust's 48.4%. [stated] Flöck dissertation Table 2
  and ch. 4 related work
- **Runtime:** "distinctly better" than A3. [stated] TokTrack
- **Moves:** paragraph and sentence moves come for free, because hash lookup
  ignores position. Word-level moves inside changed sentences rely on difflib,
  which is order-preserving.

### Rust crates (crates.io, checked 2026-11-18)

| Crate                           | Version / updated   | What it actually provides                                                                                                                                                                                                                                                                                                                                                                                                         | Moves?                        |
| ------------------------------- | ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- |
| `similar`                       | 3.2.1 / 2026-10-24  | `Algorithm::{Myers (default, heuristic), RawMyers, Patience, Lcs, Hunt, Histogram}`. Diffs slices, lines, chars, graphemes (with the `unicode` feature); `capture_diff_slices_by_key`; deadlines (the `wasm32_web_time` feature avoids an `Instant::now` panic on wasm). [stated] https://docs.rs/similar                                                                                                                         | No                            |
| `imara-diff`                    | 0.2.0 / 2025-06-14  | Linear-space Myers with heuristics, and Histogram ("ported from git and heavily optimized", "outperforms Myers diff by 10%–100%", "up to 30 times better than `similar`" by the authors' own benchmark). Token interning via `InternedInput`/`TokenSource`; `Diff::compute_with` on pre-tokenized lists; slider postprocessing. [stated] https://docs.rs/imara-diff                                                               | No                            |
| `gix-diff`                      | 0.70.0 / 2026-10-22 | Gitoxide's blob/tree diff. Has a `wasm` feature toggle "to allow compilation to wasm32-unknown-unknown" (CHANGELOG). [stated] Whether its blob diff wraps imara-diff I did not verify.                                                                                                                                                                                                                                            | No                            |
| `heckel-diff`                   | 1.0.0 / 2026-03-12  | Heckel 1978 on slices; immature. [stated]                                                                                                                                                                                                                                                                                                                                                                                         | Unverified                    |
| `wikiwho` (`Schuwi/wikiwho_rs`) | 0.3.6 / 2026-07-12  | Rust port of WikiWho with separate MediaWiki XML parser and algorithm modules. Claims "exact token-level parity against the reference Python WikiWho", plus ≥85% precision on the paper's gold standard with its native diff (95% with the `python-diff` feature). Output per token: `o_rev_id`, editor, `in`/`out` revision lists. Versions ≤0.3.4 had an entity-truncation bug. WASM-compatibility not checked. [stated] README | Paragraph/sentence level only |
| `dissimilar`                    | 1.0.11              | Google diff-match-patch port (character-level, semantic cleanup).                                                                                                                                                                                                                                                                                                                                                                 | No                            |
| `diff`, `diffs`                 | old                 | LCS/Myers basics.                                                                                                                                                                                                                                                                                                                                                                                                                 | No                            |

### Recommendation [inferred, my design judgment]

Split the problem in two. History flow's own architecture appears to have done
the same: pairwise Heckel for ribbons, history-wide identity for color.

**A. Attribution: WikiWho-style hierarchical, history-wide hash matching.**

1. Tokenize each version into paragraphs → sentences → words, with a pluggable
   tokenizer per data source:
   - Wikipedia: wikitext rules as in WikiWho
   - git: lines as "sentences"
   - prose files: sentences
2. Keep a history-wide `HashMap<hash(paragraph|sentence), Vec<TokenRun>>` of
   every unit ever seen, each carrying per-token origin IDs.
3. For each new version, match units first against the previous version, then
   against history. Matched units inherit their token identities, and therefore
   origin author and time. This gives the "restored text keeps color" behavior
   of the original and is robust to reverts and partial reverts.
4. For unmatched sentences, run a word-level diff, using `imara-diff` Histogram,
   against the unmatched words of the previous version. Optionally also run a de
   Alfaro-style n-gram lookup (n = 4) against dead text, to catch word-level
   partial restores.
5. Keep per-token `in`/`out` revision lists. These give "age" (time since
   origin) and "recent change" for free.
6. Offer a **"faithful 2004 mode"**: sentence-only tokens split on `.` and
   `<…>`, with no word-level step. It reproduces the
   comma-changes-the-whole-sentence artifact.

**B. Ribbons: pairwise alignment derived from token identity, not from a second
diff.**

- Because every token carries a stable ID, the ribbons between versions _i_ and
  _i+1_ are simply the maximal runs of token IDs that are contiguous in both
  versions. Runs that change order are moves and draw as crossing ribbons.
  Tokens absent from the neighbor are gaps.
- This is exactly Heckel's notion of linked lines, but computed from identity,
  so it is linear and consistent with the coloring by construction.
- A token restored from an older version has an ID that is not present in _i_,
  so it shows a gap on the left while keeping its old color. That matches CHI04
  Fig. 4.

**C. Why not the others alone:**

- Myers, patience and histogram alone cannot show crossing ribbons and
  mis-attribute restored text. Use them only as the inner word-level step.
- Pure Heckel matches the original but cannot do reintroduction and degrades on
  repeated tokens.
- Pure de Alfaro A3 gives attribution but no alignment, and it measured less
  precise (0.77 against 0.95) on the only published gold standard.
- WikiTrust's greedy quality heuristic is tuned for Italian Wikipedia and
  measured at about 48–50%.

**D. Complexity.**

- Per version: O(size of version) hashing, plus a word-level diff only over
  unmatched sentences. That is small for typical edits.
- Memory: grows with total distinct content. Add de Alfaro-style aging to prune
  units unseen for N revisions or T days if needed.
- Thousands of versions of a 50 KB article should be fine in WASM. This is an
  estimate, not measured.

**E. Reuse option.** For Wikipedia, `wikiwho` (Rust) or the hosted WikiWho API
(https://wikiwho-api.wmcloud.org/, 70 languages; endpoint details not verified)
can serve as a reference oracle in tests.

---

## 8. Data access

### MediaWiki Action API (Wikipedia)

**Revisions with content**

```
GET https://en.wikipedia.org/w/api.php?action=query&prop=revisions&titles=Chocolate
    &rvprop=ids|timestamp|user|userid|comment|size|sha1|content
    &rvslots=main&rvlimit=max&rvdir=newer&format=json&formatversion=2&origin=*
```

**Batch size**

- `rvlimit` range is 1–500. "If rvprop=content… is used, the limit is 50."
  [stated] https://www.mediawiki.org/wiki/API:Revisions
- [probe] The response above returned 50 revisions and
  `"limits":{"revisions":50}`.

**Continuation**

- The response includes
  `"continue":{"rvcontinue":"20030820134120|1318623","continue":"||"}`. Merge it
  into the next request. [probe; stated generally on API:Revisions]

**Single page only**

- `rvlimit`, `rvdir`, `rvstart`/`rvend` and `rvstartid`/`rvendid` work only for
  one page. [stated] API:Revisions
- `rvend=2003-09-01T00:00:00Z` reproduces the paper's era.

**Hidden revisions**

- Revision-deleted users, comments or text come back as `userhidden`,
  `commenthidden` and so on. Handle missing content. [stated] API:Revisions

**Metadata-only pass**

- Without `content` you get up to 500 revisions per request. Fetch metadata
  first to size the job. [stated]

**CORS**

- "Unauthenticated CORS requests may be made from any origin by setting the
  `origin` request parameter to `*`." `origin` must be in the query string.
  [stated] https://www.mediawiki.org/wiki/API:Cross-site_requests
- [probe] Response headers: `access-control-allow-origin: *`,
  `access-control-allow-credentials: false`.

**User-Agent**

- Wikimedia requires a descriptive User-Agent; generic ones may get HTTP 403.
- Browser JavaScript cannot set `User-Agent`, so it is "encouraged to include
  the `Api-User-Agent` header", identifying the tool with a link to its source.
  [stated]
  https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy
- Note that a custom header triggers a CORS preflight. [inferred]

**Rate limits (new in 2026, "subject to experimentation and change")**

- Limits are per minute:
  - unidentified clients (IP only): **10 req/min**
  - unauthenticated browser requests: **200 req/min**
  - unauthenticated bots with a compliant User-Agent: **200 req/min**
- Applies to both the Action and REST APIs. Overruns return HTTP 429.
- [stated] https://www.mediawiki.org/wiki/Wikimedia_APIs/Rate_limits
- [probe] My scripted run hit **HTTP 429 after about 12 requests in a minute**
  with a UA lacking an email or URL. That is consistent with the 10/min
  "unidentified" tier.
- Etiquette: make requests serially, send `Accept-Encoding: gzip`, and use
  `maxlag` for non-interactive jobs. [stated]
  https://www.mediawiki.org/wiki/API:Etiquette
- Throughput estimate: at 50 revisions per request, a 5,000-revision article is
  about 100 requests, roughly 30 s to 1 min at 200/min. [inferred]

**Alternatives**

- **XML dumps** (`pages-meta-history`) are best for offline batch processing.
  This is how the original and WikiWho/TokTrack worked. [stated] CHI04; TokTrack
- **`Special:Export`** with history: used by `rdmpage/wikihistoryflow`; its
  README notes the export XML namespace version is hard-coded and changes over
  time. [stated] https://github.com/rdmpage/wikihistoryflow. The current
  per-request revision caps are not verified.
- **WikiWho API**: precomputed token provenance. [stated existence; endpoint
  details unverified]

### Git repositories

**Option 1: offline CLI or server preprocessing (native Rust with `gix`, or
plain `git`)** [inferred]

- How: walk `git log --follow -- <path>` (or the whole repo) and emit a compact
  "document versions" bundle (JSON, CBOR or a custom binary) that the browser
  loads.
- Pros:
  - full speed
  - no CORS or auth problems
  - handles huge repos
  - can precompute tokenization and attribution too
- Cons: an extra step for users; a hosted server would need to clone arbitrary
  repos.

**Option 2: `isomorphic-git` in the browser** [stated]
https://isomorphic-git.org/docs/en/clone, https://isomorphic-git.org/docs/en/log

- What it offers:
  - a pure-JS git
  - `clone` with `corsProxy`, `depth`, `singleBranch`, `noCheckout`, `since`
  - the docs' example uses `https://cors.isomorphic-git.org`
  - `log({filepath, follow})` gives single-file history across renames, plus
    `includeChanges`
  - a filesystem backend such as LightningFS (IndexedDB)
- Pros: works today without WASM git.
- Cons:
  - GitHub and most hosts do not send CORS headers for the git smart-HTTP
    protocol, so a proxy is required [inferred; the docs show the proxy but do
    not explain why]
  - full-history clones of large repos are slow and memory-heavy in JS
    [inferred]

**Option 3: `gix` compiled to WASM** [stated]
https://github.com/GitoxideLabs/gitoxide/issues/463

- Status:
  - Several gitoxide crates compile to `wasm32-unknown-unknown`, and CI has a
    WASM job, but WASM failures are "informational" (DEVELOPMENT.md).
  - `gix-diff` and `gix-pack` have `wasm` feature toggles. `gix-url`, `gix-date`
    and `gix-glob` list `wasm32-unknown-unknown` targets. `gix-fs`, `gix-path`
    and `gix-index` gained `wasm32-wasip2` support.
  - Issue #463 "WASM support" was closed 2026-07-22 and moved to Feature
    Proposal Discussion #2772. A 2025-04 comment reports an experimental fork
    running `gix-blame` in a browser via WASI with memmap replaced by `Vec<u8>`.
    The maintainer was unsure about merging without a committed user.
  - **The top-level `gix` crate is not shown as supporting
    wasm32-unknown-unknown.** There is no network transport in the browser.
- Practical path [inferred]: read a repository the user selects locally (File
  System Access API directory picker, or an uploaded `.git` or bundle), feed
  object and pack bytes to the wasm-compatible lower-level crates (`gix-pack`,
  `gix-object`, `gix-diff`), and walk history yourself. This is feasible but
  needs experimental integration work.

**Option 4: hosting REST APIs (for example GitHub's per-file commits plus raw
blob fetches)** [inferred, not researched]

- Simple, but unauthenticated rate limits are low and the approach is per-host.

**Overall recommendation** [inferred]

- Ship Option 1 (a small Rust CLI using `gix` that writes a versions bundle) as
  the reliable path.
- Add Option 2 or 3 later for zero-install browser use on small repos.
- Wikipedia can be fetched directly from the browser (CORS works), with the
  `Api-User-Agent` header and serial, backoff-aware paging.

---

## Existing reimplementations and relatives

- **rdmpage/wikihistoryflow** (PHP, 48★, last push 2016): fetches
  `Special:Export/<title>?history`, splits text into **lines**, diffs
  consecutive versions with PEAR Text_Diff (LCS, no moves), and renders SVG.
  Clicking columns or rows shows revisions or contributors. [stated]
  https://github.com/rdmpage/wikihistoryflow
- **adotg/git-history-flow** (JavaScript/D3/React, about 29★, 2018): "an
  implementation of the same concept as IBM's History Flow" for a single file in
  git. A bash script turns `git diff` unified hunks (`@@ -a,b +c,d @@`) into
  JSON. `Snapshot`/`ContributionHunk` classes split and shift author hunks per
  transaction; line-level, no moves. Its README warns about performance on large
  datasets. [stated] https://github.com/adotg/git-history-flow, demo
  https://adotg.github.io/git-history-flow
- **history.azurewebsites.net** ("Wikipedia History Flow") is cited by the
  Wikipedia article. Not visited. [unverified]
- **git-of-theseus** (Bernhardsson): stack plots of surviving lines by cohort
  year, author share, and Kaplan–Meier line survival, built from git blame. A
  useful contrast: aggregate cohorts, not per-version flow. [stated]
  https://github.com/erikbern/git-of-theseus
- **WikiWho / TokTrack**: per-token provenance for all of English Wikipedia
  (13.5 billion token instances). Processing took about 25 days on a 20-core,
  122 GB VM. [stated] https://arxiv.org/pdf/1703.08244
- **Seesoft** (Eick et al.; Baker & Eick 1995, cited by CHI04 as "thumbnail view
  of a program, with line-by-line coloring to indicate authorship or age"),
  **Ben Fry's _revisionist_** (cited [3]), **ThemeRiver** and **parallel
  coordinates** are cited by CHI04 as visual relatives. [stated] CHI04 p.578
- **Chromograms** (Wattenberg, Viégas, Hollenbach, INTERACT 2007): color-coded
  tiles for very long edit sequences per user. A different technique. [stated]
  https://link.springer.com/doi/10.1007/978-3-540-74800-7_23. Not read.
- **WikiDashboard** (PARC), **code_swarm**, **gource**: not researched; contrast
  only.

---

## Unverified / not found

- **"Beautiful History: Visualizing Wikipedia"** (Viégas & Wattenberg, in
  _Beautiful Visualization_, O'Reilly 2010): found the citation only and did not
  read the text. It may contain implementation details I have not seen.
- **"The Hidden Order of Wikipedia"** (2007): not read.
- **hint.fm history flow page and fernandaviegas.com:** not opened.
  bewitched.com/historyflow.html was read only through a summarizer (it gives
  the creators, the chocolate/abortion examples and a MoMA image).
- **MoMA entry** (https://www.moma.org/collection/works/110349): returned
  HTTP 403. Details known only from search snippets: _History Flow_, 2003,
  chromogenic prints titled "abortion" and "chocolate", 28 × 36 in., gift of the
  designers; shown in _A Collection of Ideas_ (2014–15).
- **The alphaWorks installer and online help** (the "user manual"): the Wayback
  CDX shows no archived installer files. The developerWorks forum threads
  (forum 489) were not archived. Therefore unknown:
  - the exact tokenizer rules (delimiter attachment, whitespace and markup
    handling)
  - the age-darkening function
  - the palette and color-assignment rule
  - what "links" mode and the "posts" tab do
  - hover versus click behavior
  - how duplicate sentences and restored text were attributed (the Fig. 4
    evidence suggests history-wide sentence identity)
  - the MediaWiki plug-in's fetch method
- **Heckel 1978 original paper:** not read directly. The algorithm description
  comes from the 1986 `hdiff` implementation's comments and code, plus the ACM
  abstract via search.
- **Myers 1986, Tichy 1984, patience and histogram specifics:** described from
  standard knowledge, not from fetched papers. They are marked [inferred] where
  used.
- **WikiWho WWW 2014 paper:** the ACM PDF returned HTML. Numbers come from
  Flöck's 2016 dissertation, which reports the same experiment.
- **`heckel-diff` move support, `gix-diff`'s internal algorithm, `wikiwho` crate
  WASM compatibility, WikiWho API endpoint list and CORS:** not checked.
- **Discrepancy:** Microsoft has 198 versions per the paper but 250 per today's
  API up to 2003-08-31. The IBM page's first author also differs. Today's live
  history is not the authors' dataset.

---

## Sources

Primary (read):

- CHI 2004 paper (PDF, full text and figures extracted):
  https://politics.media.mit.edu/papers/history_flowviegas.pdf
- IBM project pages via Wayback:
  - https://web.archive.org/web/2007/http://www.research.ibm.com/visual/projects/history_flow/
    (and `explanation.htm`, `results.htm`, `gallery.htm`, `contact.htm`)
  - images under `…/history_flow/images/` (`1.gif`–`5.gif`, `*_thumbnails.gif`,
    `ibm2version.gif`, `ibm2date.gif`, `screen_shots/islam2.gif`,
    `small/small_individual_author.gif`)
- alphaWorks pages via Wayback:
  https://web.archive.org/web/2006/http://www.alphaworks.ibm.com/tech/historyflow
  (and `/download`, `/requirements`, `/faq`, `/forum`)
- Viégas PhD thesis 2005:
  https://smg.media.mit.edu/papers/Viegas/viegas_thesis.pdf
- Talk Before You Type (HICSS 2007):
  http://hint.fm/papers/wikipedia_coordination_final.pdf
- Wattenberg's project page (via summarizer):
  https://bewitched.com/historyflow.html
- Slashdot, 2005-03-28 alphaWorks release:
  https://news.slashdot.org/story/05/03/28/054225/history-flow-shows-how-wiki-articles-evolve
- Wikipedia, "IBM History Flow tool":
  https://en.wikipedia.org/wiki/IBM_History_Flow_tool

Algorithms:

- hdiff (Heckel implementation, 1986):
  https://www.tuhs.org/Usenet/mod.sources/1986-February/000410.html
- de Alfaro & Shavlovsky, WWW 2013:
  https://archives.iw3c2.org/www2013/proceedings/p343.pdf
- Adler & de Alfaro, WWW 2007:
  https://web.archive.org/web/2008id_/http://www2007.org/papers/paper692.pdf
- Flöck dissertation (WikiWho evaluation):
  https://publikationen.bibliothek.kit.edu/1000058165/3879557
- TokTrack: https://arxiv.org/pdf/1703.08244
- WikiWho source: https://github.com/wikiwho/WikiWho (development moved to
  https://github.com/wikimedia/WikiWho); API: https://wikiwho-api.wmcloud.org/
- Nugroho, Hata, Matsumoto, "How different are different diff algorithms in
  Git?": https://arxiv.org/abs/1902.02467

Rust crates:

- https://crates.io/crates/similar, https://docs.rs/similar,
  https://docs.rs/similar/latest/similar/enum.Algorithm.html
- https://crates.io/crates/imara-diff, https://docs.rs/imara-diff
- https://crates.io/crates/gix-diff, https://github.com/GitoxideLabs/gitoxide
  (issue #463, DEVELOPMENT.md, CHANGELOGs)
- https://crates.io/crates/heckel-diff, https://docs.rs/heckel-diff
- https://crates.io/crates/wikiwho, https://github.com/Schuwi/wikiwho_rs
- https://crates.io/crates/dissimilar

Data access:

- https://www.mediawiki.org/wiki/API:Revisions
- https://www.mediawiki.org/wiki/API:Cross-site_requests
- https://www.mediawiki.org/wiki/API:Etiquette
- https://www.mediawiki.org/wiki/Wikimedia_APIs/Rate_limits
- https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy
- https://isomorphic-git.org/docs/en/clone,
  https://isomorphic-git.org/docs/en/log

Reimplementations and relatives:

- https://github.com/rdmpage/wikihistoryflow
- https://github.com/adotg/git-history-flow
- https://github.com/erikbern/git-of-theseus
- https://link.springer.com/doi/10.1007/978-3-540-74800-7_23 (Chromograms)
