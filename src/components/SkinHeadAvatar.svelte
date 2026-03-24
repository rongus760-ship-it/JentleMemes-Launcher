<script lang="ts">
  /**
   * Маленькая «голова» из полной PNG-текстуры скина (не развёртка целиком в квадрате).
   * Для helm-URL из minotar используйте обычный <img>.
   */
  export let src: string;
  /** Логический размер в CSS px */
  export let size = 32;
  /** Классы обёртки (форма, кольцо и т.д.) */
  export let wrapperClass = "";
  export let alt = "Аватар";

  let canvas: HTMLCanvasElement;
  let loadGen = 0;

  function paintOnto(c: HTMLCanvasElement, textureSrc: string, gen: number) {
    const img = new Image();
    img.onload = () => {
      if (gen !== loadGen) return;
      const iw = img.naturalWidth;
      const ih = img.naturalHeight;
      if (iw < 1 || ih < 1) return;

      const baseW = 64;
      const baseH = ih >= 64 ? 64 : 32;
      const sx = (x: number) => (x / baseW) * iw;
      const sy = (y: number) => (y / baseH) * ih;
      const sw = (w: number) => (w / baseW) * iw;
      const sh = (h: number) => (h / baseH) * ih;

      const d = Math.max(1, Math.floor(size));
      c.width = d;
      c.height = d;
      const ctx = c.getContext("2d");
      if (!ctx) return;
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, d, d);
      ctx.drawImage(img, sx(8), sy(8), sw(8), sh(8), 0, 0, d, d);
      if (ih >= 64) {
        ctx.drawImage(img, sx(40), sy(8), sw(8), sh(8), 0, 0, d, d);
      }
    };
    img.onerror = () => {
      if (gen !== loadGen) return;
      const d = Math.max(1, Math.floor(size));
      c.width = d;
      c.height = d;
      const ctx = c.getContext("2d");
      if (ctx) {
        ctx.fillStyle = "rgba(0,0,0,0.25)";
        ctx.fillRect(0, 0, d, d);
      }
    };
    img.src = textureSrc;
  }

  $: if (canvas && src) {
    loadGen++;
    paintOnto(canvas, src, loadGen);
  }
</script>

<div
  role="img"
  aria-label={alt}
  class="inline-block overflow-hidden shrink-0 {wrapperClass}"
  style:width="{size}px"
  style:height="{size}px"
>
  <canvas
    bind:this={canvas}
    class="block w-full h-full"
    style:image-rendering="pixelated"
    aria-hidden="true"
  />
</div>
