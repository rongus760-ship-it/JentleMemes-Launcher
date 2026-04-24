<script lang="ts">
  /** PNG (URL или data URL) */
  export let src: string;
  export let size = 200;
  /** classic = Стив (4px руки), slim = Алекс (3px) */
  export let model: "default" | "slim" = "default";
  /** Второй слой (шляпа, куртка, штаны, рукава) — как в 3D Minecraft */
  export let showOverlay = true;

  let canvas: HTMLCanvasElement;

  function paint() {
    if (!canvas || !src) return;
    const img = new Image();
    /** Без CORS: иначе Ely.by / skinsystem не отдают ACAO — img грузится, canvas только «tainted» (для превью ок). */
    img.onload = () => {
      const iw = img.naturalWidth;
      const ih = img.naturalHeight;
      if (iw < 1 || ih < 1) return;

      const baseW = 64;
      const baseH = ih >= 64 ? 64 : 32;
      const sx = (x: number) => (x / baseW) * iw;
      const sy = (y: number) => (y / baseH) * ih;
      const sw = (w: number) => (w / baseW) * iw;
      const sh = (h: number) => (h / baseH) * ih;

      const scale = Math.max(1, Math.floor(size / 32));
      const armW = model === "slim" ? 3 : 4;
      const bodyW = 8;
      const totalW = armW + bodyW + armW;
      /** 1px «подушки» вокруг каждой части — чтобы увеличенный 2-й слой не обрезался. */
      const PAD = scale;
      const w = totalW * scale + PAD * 2;
      const h = 32 * scale + PAD * 2;
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, w, h);

      const ox = PAD;
      const oy = PAD;
      const ax0 = 0 + ox;
      const bx = armW * scale + ox;
      const ax1 = (armW + bodyW) * scale + ox;
      const hx = armW * scale + ox;

      const draw = (
        u0: number,
        v0: number,
        uw: number,
        vh: number,
        dx: number,
        dy: number,
        dw: number,
        dh: number,
      ) => {
        ctx.drawImage(img, sx(u0), sy(v0), sw(uw), sh(vh), dx, dy, dw, dh);
      };

      /**
       * Inflate: overlay в реальном Minecraft рендерится масштабом 9/8 вокруг базовой кости —
       * благодаря этому «шляпа / куртка» визуально видны отдельным слоем. Тут эмулируем 2D-версию:
       * overlay шире базы на ~1 scaled пиксель с каждой стороны.
       */
      const INFLATE_PX = Math.max(1, Math.round(scale * 0.5));

      const drawInflated = (
        u0: number,
        v0: number,
        uw: number,
        vh: number,
        dx: number,
        dy: number,
        dw: number,
        dh: number,
      ) => {
        ctx.drawImage(
          img,
          sx(u0),
          sy(v0),
          sw(uw),
          sh(vh),
          dx - INFLATE_PX,
          dy - INFLATE_PX,
          dw + INFLATE_PX * 2,
          dh + INFLATE_PX * 2,
        );
      };

      const srcArmL = model === "slim" ? [36, 52, 3, 12] : [36, 52, 4, 12];
      const srcArmR = model === "slim" ? [46, 20, 3, 12] : [44, 20, 4, 12];
      const olArmL = model === "slim" ? [52, 52, 3, 12] : [52, 52, 4, 12];
      const olArmR = model === "slim" ? [46, 36, 3, 12] : [44, 36, 4, 12];

      // base layer
      draw(8, 8, 8, 8, hx, oy + 0, 8 * scale, 8 * scale);
      draw(20, 20, 8, 12, bx, oy + 8 * scale, 8 * scale, 12 * scale);
      draw(srcArmL[0], srcArmL[1], srcArmL[2], srcArmL[3], ax0, oy + 8 * scale, armW * scale, 12 * scale);
      draw(srcArmR[0], srcArmR[1], srcArmR[2], srcArmR[3], ax1, oy + 8 * scale, armW * scale, 12 * scale);
      draw(4, 20, 4, 12, bx, oy + 20 * scale, 4 * scale, 12 * scale);
      draw(20, 52, 4, 12, bx + 4 * scale, oy + 20 * scale, 4 * scale, 12 * scale);

      if (showOverlay && baseH >= 64) {
        // overlay layer — слегка раздут, чтобы было видно, что это именно «второй слой»
        drawInflated(20, 36, 8, 12, bx, oy + 8 * scale, 8 * scale, 12 * scale);
        drawInflated(olArmL[0], olArmL[1], olArmL[2], olArmL[3], ax0, oy + 8 * scale, armW * scale, 12 * scale);
        drawInflated(olArmR[0], olArmR[1], olArmR[2], olArmR[3], ax1, oy + 8 * scale, armW * scale, 12 * scale);
        drawInflated(4, 36, 4, 12, bx, oy + 20 * scale, 4 * scale, 12 * scale);
        drawInflated(4, 52, 4, 12, bx + 4 * scale, oy + 20 * scale, 4 * scale, 12 * scale);
      }
      if (showOverlay && iw >= 48 && ih >= 16) {
        drawInflated(40, 8, 8, 8, hx, oy + 0, 8 * scale, 8 * scale);
      }
    };
    img.onerror = () => {};
    img.src = src;
  }

  $: {
    src;
    size;
    model;
    showOverlay;
    if (canvas) paint();
  }
</script>

<canvas
  bind:this={canvas}
  class="drop-shadow-2xl"
  style:image-rendering="pixelated"
  style:width="{(model === 'slim' ? 14 : 16) * Math.max(1, Math.floor(size / 32)) + 2 * Math.max(1, Math.floor(size / 32))}px"
  style:height="{32 * Math.max(1, Math.floor(size / 32)) + 2 * Math.max(1, Math.floor(size / 32))}px"
/>
