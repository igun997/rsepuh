// rsepuh compositor. Runs inside effectcraft-cli (After Effects style scripting).
// rsepuh injects, before this file: SPEC (the project spec), THEME (the resolved style),
// and ROOT (project folder). Everything below only uses the public
// scripting model, so a user can copy any helper into their own custom scene.
//
// Motion principles used throughout: fast-in / slow-out easing, springs that overshoot and settle,
// things arrive from somewhere (rise, tilt, slide) and keep a little life afterwards (float),
// every scene sits on a camera rig (slow push, entrance and exit), and cuts overlap the moves.
(function () {
  var W = SPEC.size[0], H = SPEC.size[1], FPS = SPEC.fps;
  var S = Math.min(W, H) / 1080; // sizes below are written for a 1080 px short side
  var scenes = SPEC.scenes;

  // ---------------------------------------------------------------- timeline
  var total = 0, i;
  for (i = 0; i < scenes.length; i++) {
    if (scenes[i].at === undefined) scenes[i].at = total;
    total = Math.max(total, scenes[i].at + scenes[i].dur);
  }
  if (SPEC.duration) total = Math.max(total, SPEC.duration);
  var comp = app.project.items.addComp("Main", W, H, 1, total, FPS);
  comp.bgColor = rgb(SPEC.bg || THEME.bg);
  try { comp.motionBlur = !!THEME.motionBlur; } catch (e) {}

  // ---------------------------------------------------------------- helpers
  function rgb(h) {
    h = String(h).replace("#", "");
    if (h.length === 3) h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2];
    return [parseInt(h.substr(0, 2), 16) / 255, parseInt(h.substr(2, 2), 16) / 255, parseInt(h.substr(4, 2), 16) / 255];
  }
  function P(l, n) { return l.property("ADBE Transform Group").property(n); }
  function dims(p) { var v = p.value; return v instanceof Array ? v.length : 1; }
  function easeKey(p, k, inf, outf) {
    var n = dims(p), a = [], b = [], d;
    for (d = 0; d < n; d++) { a.push(new KeyframeEase(0, inf)); b.push(new KeyframeEase(0, outf)); }
    try { p.setTemporalEaseAtKey(k, a, b); } catch (e) {}
  }
  function ease(p, inf, outf) { for (var k = 1; k <= p.numKeys; k++) easeKey(p, k, inf, outf); }
  // keys: [[t, value], ...] with a symmetric ease
  function anim(p, keys, inf) {
    for (var k = 0; k < keys.length; k++) p.setValueAtTime(keys[k][0], keys[k][1]);
    ease(p, inf || 60, inf || 60);
  }
  // fast-in / slow-out: the first key leaves quickly, the last arrives gently, middle keys flow through
  function settle(p, keys, soft) {
    var base = p.numKeys, k, n = keys.length;
    for (k = 0; k < n; k++) p.setValueAtTime(keys[k][0], keys[k][1]);
    for (k = 0; k < n; k++) {
      var inf = k === 0 ? 33 : (k === n - 1 ? (soft || 88) : 55);
      var outf = k === 0 ? 12 : 55;
      easeKey(p, base + k + 1, inf, outf);
    }
  }
  function span(l, t0, t1) { l.inPoint = t0; l.outPoint = t1; return l; }
  var CUR = null;
  function useCaps() { return CUR && CUR.caps !== undefined ? !!CUR.caps : !!THEME.caps; }
  function caps(s) { return useCaps() ? String(s).toUpperCase() : String(s); }
  function q(s) { return '"' + String(s).replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n") + '"'; }

  function solid(color, t0, t1, name) {
    var l = comp.layers.addSolid(rgb(color), name || "solid", W, H, 1);
    return span(l, t0, t1);
  }
  function gradient(c1, c2, t0, t1, radial) {
    var l = solid(c1, t0, t1, "bg");
    var e = l.property("ADBE Effect Parade").addProperty("ec.generate.gradientramp");
    e.property("Start of Ramp").setValue(radial ? [W / 2, H * 0.4] : [0, 0]);
    e.property("Start Color").setValue(rgb(c1).concat([1]));
    e.property("End of Ramp").setValue(radial ? [W * 1.1, H * 0.95] : [0, H]);
    e.property("End Color").setValue(rgb(c2).concat([1]));
    try { e.property("Ramp Shape").setValue(radial ? 2 : 1); } catch (x) {}
    return l;
  }
  // soft coloured light that drifts behind everything: depth instead of a flat colour
  function aurora(t0, t1, seed) {
    if (!THEME.aurora) return;
    var cols = [THEME.accent, THEME.accent2, THEME.accent], k;
    for (k = 0; k < 3; k++) {
      var sh = comp.layers.addShape();
      sh.name = "bgfx";
      var g = sh.property("ADBE Root Vectors Group").addProperty("ADBE Vector Group");
      var e = g.property("ADBE Vectors Group").addProperty("ADBE Vector Shape - Ellipse");
      e.property("ADBE Vector Ellipse Size").setValue([(700 + k * 140) * S, (700 + k * 140) * S]);
      var f = g.property("ADBE Vectors Group").addProperty("ADBE Vector Graphic - Fill");
      f.property("ADBE Vector Fill Color").setValue(rgb(cols[k]).concat([1]));
      P(sh, "ADBE Opacity").setValue(THEME.aurora * 100 * (k === 1 ? 0.8 : 1));
      var bl = sh.property("ADBE Effect Parade").addProperty("ec.blur.gaussian");
      bl.property("Blurriness").setValue(220 * S);
      var ph = seed * 1.7 + k * 2.1;
      P(sh, "ADBE Position").expression =
        "[" + (W * (0.2 + 0.3 * k)) + "+Math.sin(time*0.31+" + ph + ")*" + (W * 0.2) + "," + (H * (0.22 + 0.26 * k)) + "+Math.cos(time*0.24+" + ph + ")*" + (H * 0.1) + "]";
      span(sh, t0, t1);
    }
  }
  function background(t0, t1, sc) {
    var c1 = sc.bg || THEME.bg, c2 = sc.bg2 || THEME.bg2;
    var l = c1 === c2 ? solid(c1, t0, t1, "bg") : gradient(c1, c2, t0, t1, true);
    aurora(t0, t1, t0);
    return l;
  }

  // text: centred on (x, y) with its anchor in the middle, so scale pops grow from the centre.
  function text(str, o) {
    var l = comp.layers.addText(o.raw ? str : caps(str));
    var p = l.property("ADBE Text Properties").property("ADBE Text Document");
    var d = p.value;
    d.font = o.font || THEME.font;
    d.fontSize = (o.size || 100) * S;
    d.fillColor = rgb(o.color || THEME.fg);
    d.justification = o.left ? ParagraphJustification.LEFT_JUSTIFY : ParagraphJustification.CENTER_JUSTIFY;
    var stroke = o.stroke === undefined ? THEME.stroke : o.stroke;
    if (stroke) {
      d.applyStroke = true;
      d.strokeColor = rgb(stroke);
      d.strokeWidth = (o.strokeW || THEME.strokeW || 8) * S;
      try { d.strokeOverFill = false; } catch (e) {}
    }
    d.tracking = o.tracking === undefined ? THEME.tracking || 0 : o.tracking;
    p.setValue(d);
    l.__doc = p;
    l.__left = !!o.left;
    l.__base = !!o.baseline;
    place(l, o.x === undefined ? W / 2 : o.x, o.y === undefined ? H / 2 : o.y, o.left);
    return l;
  }
  // baseline text (code, terminal): y is the baseline, so punctuation and letters share one line
  function place(l, x, y, left) {
    var r = l.sourceRectAtTime(0, false);
    P(l, "ADBE Anchor Point").setValue([left ? r.left : r.left + r.width / 2, l.__base ? 0 : r.top + r.height / 2]);
    P(l, "ADBE Position").setValue([x, y]);
    l.__w = r.width; l.__h = r.height;
  }
  function fit(l, maxW, maxSize) {
    var p = l.__doc, d = p.value;
    var r = l.sourceRectAtTime(0, false);
    var size = d.fontSize * (maxW / Math.max(r.width, 1));
    d.fontSize = Math.min(size, (maxSize || 9999) * S);
    p.setValue(d);
    var pos = P(l, "ADBE Position").value;
    place(l, pos[0], pos[1], l.__left);
  }

  // ---- motion
  // position expression assembled from shake and idle float, so they can live on one layer
  function posExpr(l) {
    var parts = ["value"], o = l.__mo || {};
    if (o.shake) parts.push("(function(){var t=time-" + o.shake[0] + "; return t>=0 && t<" + o.shake[1] + " ? (wiggle(18," + o.shake[2] + ")-value)*(1-t/" + o.shake[1] + ") : [0,0];})()");
    if (o.float) parts.push("[Math.sin(time*" + o.float[2] + "+" + o.float[3] + ")*" + o.float[0] + ",Math.cos(time*" + (o.float[2] * 0.82) + "+" + o.float[3] + ")*" + o.float[1] + "]");
    P(l, "ADBE Position").expression = parts.join(" + ");
  }
  function idle(l, ax, ay, period, phase) {
    l.__mo = l.__mo || {};
    l.__mo.float = [ax * S, ay * S, (2 * Math.PI) / (period || 4), phase || 0];
    posExpr(l);
  }
  // scale spring: arrives from `from` %, overshoots to `over` x, dips, settles at 100
  function springScale(l, t, d, from, over) {
    var o = over * 100;
    settle(P(l, "ADBE Scale"), [[t, [from, from]], [t + d * 0.5, [o, o]], [t + d * 0.78, [98.4, 98.4]], [t + d, [100, 100]]], 80);
  }
  function pop(l, t, d, over, noFade) {
    springScale(l, t, d * 1.25, 72, over || THEME.pop);
    if (!noFade) settle(P(l, "ADBE Opacity"), [[t, 0], [t + Math.min(0.14, d * 0.5), 100]], 50);
  }
  function flash(t, color, d) {
    var l = solid(color || "#ffffff", t, t + (d || 0.14), "flash");
    anim(P(l, "ADBE Opacity"), [[t, 85], [t + (d || 0.14), 0]], 40);
    return l;
  }
  function pill(w, h, color, name) {
    var sh = comp.layers.addShape();
    sh.name = name || "pill";
    var g = sh.property("ADBE Root Vectors Group").addProperty("ADBE Vector Group");
    var r = g.property("ADBE Vectors Group").addProperty("ADBE Vector Shape - Rect");
    r.property("ADBE Vector Rect Size").setValue([w, h]);
    r.property("ADBE Vector Rect Roundness").setValue(Math.min(w, h) / 2);
    var f = g.property("ADBE Vectors Group").addProperty("ADBE Vector Graphic - Fill");
    f.property("ADBE Vector Fill Color").setValue(rgb(color).concat([1]));
    P(sh, "ADBE Position").setValue([W / 2, H / 2]);
    return sh;
  }
  function box(w, h, round, color, x, y, name) {
    var sh = pill(w, h, color, name);
    var r = sh.property("ADBE Root Vectors Group").property(1).property("ADBE Vectors Group").property("ADBE Vector Shape - Rect");
    r.property("ADBE Vector Rect Roundness").setValue(round);
    P(sh, "ADBE Position").setValue([x, y]);
    return sh;
  }

  function importAsset(path, seq) {
    var io = new ImportOptions(new File(ROOT + "/" + path));
    if (seq) io.sequence = true;
    var it = app.project.importFile(io);
    if (seq) { try { it.mainSource.conformFrameRate = FPS; } catch (e) {} }
    return it;
  }
  function cover(l, it, t0, t1, zoom) {
    var sc = Math.max(W / it.width, H / it.height) * 100;
    var z = 1 + (zoom === undefined ? 0.08 : zoom);
    P(l, "ADBE Position").setValue([W / 2, H / 2]);
    anim(P(l, "ADBE Scale"), [[t0, [sc, sc]], [t1, [sc * z, sc * z]]], 30);
  }
  function mediaLayer(src, t0, t1, seq) {
    var it = importAsset(src, seq);
    var l = comp.layers.add(it);
    l.startTime = t0;
    span(l, t0, t1);
    l.__it = it;
    return l;
  }

  // ---------------------------------------------------------------- scenes: background, footage, Lottie motion
  // The text, illustrations and UI motion of every scene is a Lottie file written by lottie/writer.py
  // and imported before this script runs. Here each scene gets its background (or its footage), and the
  // Lottie comp is laid over it.
  function pad2(n) { return n < 10 ? "0" + n : "" + n; }
  function lottieItem(i) {
    var nm = "rs-" + SPEC.name + "-s" + pad2(i);
    for (var k = 1; k <= app.project.numItems; k++) if (app.project.item(k).name === nm) return app.project.item(k);
    throw new Error("the motion writer produced no Lottie for scene " + (i + 1) + " (" + nm + ")");
  }
  function sceneLayers(i, sc, t0, t1) {
    var media = sc.type === "media";
    if (media && sc.src) {
      var m = mediaLayer(sc.src, t0, t1, sc.seq);
      m.name = "bg";
      cover(m, m.__it, t0, t1, sc.zoom);
      if (sc.dim !== 0) {
        var dim = solid("#000000", t0, t1, "bg");
        P(dim, "ADBE Opacity").setValue((sc.dim === undefined ? 0.28 : sc.dim) * 100);
      }
    } else if (sc.bg !== false) {
      background(t0, t1, sc);
    }
    if (media && sc.overlay) {
      var o = mediaLayer(sc.overlay, t0, t1, sc.overlaySeq);
      var size = (sc.overlaySize || 0.72) * W / o.__it.width * 100;
      P(o, "ADBE Position").setValue([W / 2, H * (sc.overlayY || 0.42)]);
      var ot = t0 + 0.1;
      settle(P(o, "ADBE Scale"), [[ot, [size * 0.55, size * 0.55]], [ot + 0.3, [size * 1.07, size * 1.07]], [ot + 0.48, [size * 0.985, size * 0.985]], [ot + 0.65, [size, size]]], 80);
      settle(P(o, "ADBE Opacity"), [[ot, 0], [ot + 0.2, 100]], 50);
      idle(o, 0, 16, 4.4, 0);
      try {
        var ds = o.property("ADBE Effect Parade").addProperty("ADBE Drop Shadow");
        ds.property("Opacity").setValue(60); ds.property("Distance").setValue(24 * S); ds.property("Softness").setValue(60 * S);
      } catch (e) {}
    }
    var item = lottieItem(i), l = comp.layers.add(item);
    l.name = "motion " + (i + 1);
    l.startTime = t0;
    span(l, t0, t1);
    P(l, "ADBE Position").setValue([W / 2, H / 2]);
    var k = (sc.fit === "cover" ? Math.max : Math.min)(W / item.width, H / item.height) * 100;
    P(l, "ADBE Scale").setValue([k, k]);
  }

  // ---------------------------------------------------------------- build: scenes on camera rigs
  var rigs = [];
  for (i = 0; i < scenes.length; i++) {
    var sc = scenes[i], a = sc.at, b = Math.min(total, sc.at + sc.dur);
    var before = comp.numLayers;
    sceneLayers(i, sc, a, b);
    // every layer the scene made (except its backgrounds) rides one rig null: camera moves, entrance, exit
    var made = comp.numLayers - before;
    var rig = comp.layers.addNull();
    rig.name = "rig " + (i + 1);
    P(rig, "ADBE Anchor Point").setValue([W / 2, H / 2]);
    P(rig, "ADBE Position").setValue([W / 2, H / 2]);
    span(rig, a, b);
    for (var m = 2; m <= made + 1; m++) {
      var L = comp.layer(m);
      if (L.parent === null && L.name !== "bg" && L.name !== "bgfx") L.parent = rig;
    }
    rigs.push(rig);
  }
  // ---------------------------------------------------------------- build: camera, entrance, exit, transitions
  function keysWithEase(p, list) {
    var k;
    for (k = 0; k < list.length; k++) p.setValueAtTime(list[k][0], list[k][1]);
    for (k = 0; k < list.length; k++) easeKey(p, p.numKeys - list.length + k + 1, list[k][2], list[k][3]);
  }
  for (i = 0; i < scenes.length; i++) {
    var s = scenes[i], r = rigs[i], ta = s.at, tb = Math.min(total, s.at + s.dur);
    var inTr = s.transition || THEME.transition || "cut";
    var nextTr = i + 1 < scenes.length ? (scenes[i + 1].transition || THEME.transition || "cut") : "cut";
    var cam = s.camera || THEME.camera || "push";
    var dir = i % 2 === 0 ? 1 : -1;
    var enter = (i > 0 && inTr !== "cut" && inTr !== "wipe");
    // scale: entrance settle, slow camera move, accelerating exit
    var list = [];
    if (enter) list.push([ta, [inTr === "dip" ? 100 : 94, inTr === "dip" ? 100 : 94], 33, 12], [ta + 0.6, [100, 100], 85, 40]);
    else list.push([ta, [100, 100], 33, 33]);
    var hold = cam === "none" ? 100 : 103.5;
    var exitOn = nextTr !== "cut" && nextTr !== "wipe";
    var endT = exitOn ? tb - 0.32 : tb;
    if (endT > list[list.length - 1][0] + 0.1) list.push([endT, [hold, hold], 40, exitOn ? 70 : 33]);
    if (exitOn) list.push([tb, [hold * 1.055, hold * 1.055], 22, 22]);
    keysWithEase(P(r, "ADBE Scale"), list);
    if (enter) {
      settle(P(r, "ADBE Position"), [[ta, [W / 2, H / 2 + 30 * S]], [ta + 0.6, [W / 2, H / 2]]], 88);
      settle(P(r, "ADBE Rotate Z"), [[ta, dir * 1.1], [ta + 0.7, 0]], 85);
    }
    // slow drift in rotation while the scene plays
    if (cam !== "none") {
      var rotStart = enter ? ta + 0.7 : ta;
      if (tb - rotStart > 0.4) anim(P(r, "ADBE Rotate Z"), [[rotStart, 0], [tb, dir * -0.5]], 30);
    }
  }
  // veils: a flat dip to the background colour that hides the cut between camera moves
  for (i = 0; i < scenes.length; i++) {
    var vs = scenes[i], vt0 = vs.at, vt1 = Math.min(total, vs.at + vs.dur);
    var vin = vs.transition || THEME.transition || "cut";
    var vnext = i + 1 < scenes.length ? (scenes[i + 1].transition || THEME.transition || "cut") : "cut";
    var fadeIn = i > 0 && (vin === "push" || vin === "dip");
    var fadeOut = vnext === "push" || vnext === "dip";
    if (!fadeIn && !fadeOut) continue;
    var veilColor = (i + 1 < scenes.length && vnext === "dip" && fadeOut) || (fadeIn && vin === "dip") ? "#000000" : (SPEC.bg || THEME.bg);
    var veil = solid(veilColor, vt0, vt1, "veil");
    var vkeys = [];
    if (fadeIn) vkeys.push([vt0, 100], [vt0 + 0.32, 0]); else vkeys.push([vt0, 0]);
    if (fadeOut) vkeys.push([vt1 - 0.3, 0], [vt1 - 0.02, 100]);
    anim(P(veil, "ADBE Opacity"), vkeys, 45);
  }
  for (i = 0; i < scenes.length; i++) {
    var o = scenes[i], ot = o.at, otr = o.transition || THEME.transition || "cut";
    if (i === 0 || otr === "cut" || otr === "push" || otr === "dip") continue;
    if (otr === "flash" || otr === "accent") flash(ot, otr === "accent" ? THEME.accent : "#ffffff", 0.18);
    else if (otr === "wipe") {
      var wp = solid(THEME.accent, ot - 0.22, ot + 0.22, "wipe");
      settle(P(wp, "ADBE Position"), [[ot - 0.22, [-W / 2, H / 2]], [ot, [W / 2, H / 2]], [ot + 0.22, [W * 1.5, H / 2]]], 40);
    }
  }
  if (SPEC.watermark) {
    var wm = text(SPEC.watermark, { size: 38, color: THEME.fg, y: H * 0.955, raw: true, stroke: false });
    P(wm, "ADBE Opacity").setValue(55);
    span(wm, 0, total);
  }

  // ---------------------------------------------------------------- subtitles
  var wcache = {};
  function subLayout(words, size, maxW) {
    var gap = size * 0.34 * S, rows = [], row = [], rowW = 0;
    for (var k = 0; k < words.length; k++) {
      var key = words[k] + "|" + size;
      if (wcache[key] === undefined) {
        var pr = text(words[k], { size: size, x: -5000, y: -5000, raw: true, stroke: false });
        wcache[key] = pr.__w;
        pr.remove();
      }
      var w = wcache[key];
      if (row.length && rowW + gap + w > maxW) { rows.push({ items: row, w: rowW }); row = []; rowW = 0; }
      row.push({ i: k, w: w });
      rowW += (row.length > 1 ? gap : 0) + w;
    }
    rows.push({ items: row, w: rowW });
    return { rows: rows, gap: gap };
  }

  // One cue as a phrase of per-word layers. o: size, y, upper, hot ("color" | "marker" | "glow"),
  // box / strip (backing), plain (no stroke), fadeOnly
  function phrase(cue, o) {
    var words = cue.words, texts = [], k;
    for (k = 0; k < words.length; k++) texts.push(o.upper ? words[k].w.toUpperCase() : words[k].w);
    var lay = subLayout(texts, o.size, W * 0.86), rows = lay.rows;
    var lineH = o.size * 1.22 * S, y0 = o.y - ((rows.length - 1) * lineH) / 2;
    var t0 = cue.s, t1 = cue.e, maxRow = 0, r, j;
    for (r = 0; r < rows.length; r++) maxRow = Math.max(maxRow, rows[r].w);
    if (o.box || o.strip) {
      var padX = 44 * S, padY = 26 * S;
      var bw = o.strip ? W : maxRow + padX * 2, bh = rows.length * lineH + padY * 2 - (lineH - o.size * S) * 0.5;
      var bx = box(bw, bh, o.strip ? 0 : 30 * S, "#000000", W / 2, o.y, "sub-bg");
      P(bx, "ADBE Opacity").setValue(o.strip ? 58 : 90);
      if (!o.strip) {
        try {
          var ring = bx.property("ADBE Root Vectors Group").property(1).property("ADBE Vectors Group").addProperty("ADBE Vector Graphic - Stroke");
          ring.property("ADBE Vector Stroke Color").setValue(rgb(THEME.accent).concat([1]));
          ring.property("ADBE Vector Stroke Width").setValue(5 * S);
        } catch (e) {}
      }
      span(bx, t0, t1);
    }
    for (r = 0; r < rows.length; r++) {
      var x = W / 2 - rows[r].w / 2, ry = y0 + r * lineH;
      for (j = 0; j < rows[r].items.length; j++) {
        var it = rows[r].items[j], cx = x + it.w / 2, wd = words[it.i];
        var base = text(texts[it.i], { size: o.size, color: THEME.fg, x: cx, y: ry, raw: true, stroke: o.plain ? false : undefined });
        span(base, t0, t1);
        var he = it.i + 1 < words.length ? words[it.i + 1].s : t1;
        he = Math.min(Math.max(he, wd.s + 0.1), t1);
        if (o.hot === "marker") {
          var mk = box(it.w + 30 * S, o.size * 1.08 * S, 16 * S, THEME.accent, cx, ry, "mark");
          span(mk, wd.s, he);
          var mt = text(texts[it.i], { size: o.size, color: THEME.onAccent, x: cx, y: ry, raw: true, stroke: false });
          span(mt, wd.s, he);
        } else if (o.hot) {
          var hot = text(texts[it.i], { size: o.size, color: THEME.accent, x: cx, y: ry, raw: true, stroke: o.plain ? false : undefined });
          span(hot, wd.s, he);
          pop(hot, wd.s, 0.12, 1.05);
          if (o.hot === "glow") {
            try {
              var gl = hot.property("ADBE Effect Parade").addProperty("ec.stylize.glow");
              gl.property("Glow Radius").setValue(36 * S);
              gl.property("Glow Intensity").setValue(1.4);
            } catch (e) {}
          }
        }
        x += it.w + lay.gap;
      }
    }
  }

  function subtitleCue(style, cue, o) {
    if (style === "pop") {
      for (var k = 0; k < cue.words.length; k++) {
        var wd = cue.words[k];
        var end = k + 1 < cue.words.length ? cue.words[k + 1].s : cue.e;
        var l = text(o.upper ? wd.w.toUpperCase() : wd.w, { size: 190, color: k % 2 === 0 ? THEME.fg : THEME.accent, y: o.popY, raw: true });
        fit(l, W * 0.88, 230);
        span(l, wd.s, Math.max(end, wd.s + 0.12));
        pop(l, wd.s, 0.16, 1.2);
      }
    } else if (style === "typewriter") {
      var parts = [];
      for (var q2 = 0; q2 < cue.words.length; q2++) parts.push(o.upper ? cue.words[q2].w.toUpperCase() : cue.words[q2].w);
      var str = parts.join(" ");
      var tl = text(str, { size: o.size, color: THEME.fg, y: o.y, raw: true, font: THEME.mono });
      fit(tl, W * 0.86, o.size);
      var reveal = Math.max(0.2, (cue.words[cue.words.length - 1].e - cue.s) * 0.9);
      tl.__doc.expression = "var s=" + q(str) + ";var n=Math.floor(clamp((time-" + cue.s + ")/" + reveal + "*s.length,0,s.length));s.substr(0,n)";
      span(tl, cue.s, cue.e);
    } else if (style === "outline") {
      phrase(cue, { size: o.size, y: o.y, upper: o.upper });
    } else if (style === "lower") {
      phrase(cue, { size: Math.round(o.size * 0.62), y: o.lowY, upper: o.upper, strip: true, plain: true });
    } else if (style === "box") {
      phrase(cue, { size: o.size, y: o.y, upper: o.upper, box: true, hot: "color", plain: true });
    } else if (style === "marker") {
      phrase(cue, { size: o.size, y: o.y, upper: o.upper, hot: "marker" });
    } else if (style === "glow") {
      phrase(cue, { size: o.size, y: o.y, upper: o.upper, hot: "glow" });
    } else {
      phrase(cue, { size: o.size, y: o.y, upper: o.upper, hot: "color" });
    }
  }

  var subs = SPEC.subtitles || {};
  if (subs.enabled !== false && SPEC.cues && SPEC.cues.length) {
    CUR = null;
    var vertical = H > W;
    var so = {
      size: subs.size || (vertical ? 84 : 66),
      y: H * (subs.y || (vertical ? 0.76 : 0.82)),
      lowY: H * (subs.y || (vertical ? 0.84 : 0.9)),
      popY: H * (subs.y || 0.5),
      upper: subs.caps === undefined || subs.caps === null ? !!THEME.caps : !!subs.caps
    };
    for (i = 0; i < SPEC.cues.length; i++) subtitleCue(subs.style || "karaoke", SPEC.cues[i], so);
  }

  // ---------------------------------------------------------------- audio
  for (i = 0; i < scenes.length; i++) {
    if (!scenes[i].voiceFile) continue;
    try {
      var va = comp.layers.add(importAsset(scenes[i].voiceFile, false));
      va.startTime = scenes[i].voiceAt;
    } catch (e) { writeLn("rsepuh: voice clip not added: " + e); }
  }
  if (SPEC.music) {
    try {
      var ml = comp.layers.add(importAsset(SPEC.music.file, false));
      ml.startTime = 0;
      var lv = ml.property("ADBE Audio Group").property("ADBE Audio Levels");
      var g = SPEC.music.gainDb, dk = SPEC.music.duckDb, wins = [], vw = SPEC.voiceWindows || [];
      for (i = 0; i < vw.length; i++) {
        var a0 = Math.max(0, vw[i][0] - 0.3), b0 = vw[i][1] + 0.5;
        if (wins.length && a0 <= wins[wins.length - 1][1]) wins[wins.length - 1][1] = b0; else wins.push([a0, b0]);
      }
      lv.setValueAtTime(0, [g, g]);
      for (i = 0; i < wins.length; i++) {
        var wa = wins[i][0], wb = wins[i][1];
        if (wa > 0.01) lv.setValueAtTime(wa, [g, g]);
        lv.setValueAtTime(wa + 0.3, [g + dk, g + dk]);
        lv.setValueAtTime(wb - 0.5, [g + dk, g + dk]);
        lv.setValueAtTime(wb, [g, g]);
      }
      lv.setValueAtTime(Math.max(total - 0.9, 0.5), [g, g]);
      lv.setValueAtTime(total, [-48, -48]);
    } catch (e) { writeLn("rsepuh: music not added: " + e); }
  }
  if (SPEC.sfx && SPEC.sfx.file) {
    try {
      var sl = comp.layers.add(importAsset(SPEC.sfx.file, false));
      sl.startTime = 0;
      var slv = sl.property("ADBE Audio Group").property("ADBE Audio Levels");
      slv.setValue([SPEC.sfx.gainDb, SPEC.sfx.gainDb]);
    } catch (e) { writeLn("rsepuh: sfx not added: " + e); }
  }
  if (THEME.grain) {
    var gr = comp.layers.addSolid([0, 0, 0], "grain", W, H, 1);
    gr.adjustmentLayer = true;
    var ne = gr.property("ADBE Effect Parade").addProperty("ec.noise.noise");
    try { ne.property(1).setValue(THEME.grain * 100); } catch (e) {}
    span(gr, 0, total);
  }
  writeLn("rsepuh: built " + scenes.length + " scenes, " + (Math.round(total * 100) / 100) + "s, " + comp.numLayers + " layers");
})();
