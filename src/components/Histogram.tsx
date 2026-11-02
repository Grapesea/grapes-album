import { useEffect, useRef } from "react";
import type { AssetView } from "../bindings/types";
import { thumbnailUrl } from "../api/thumbnails";

const BINS = 64;
const HEIGHT = 72;

export function Histogram({ asset }: { asset: AssetView }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const source = thumbnailUrl(asset);
    const canvas = canvasRef.current;
    if (!source || !canvas) return;

    let disposed = false;
    const image = new Image();
    image.crossOrigin = "anonymous";
    image.onload = () => {
      if (disposed) return;
      const sample = document.createElement("canvas");
      sample.width = 128;
      sample.height = 80;
      const context = sample.getContext("2d", { willReadFrequently: true });
      if (!context) return;
      context.drawImage(image, 0, 0, sample.width, sample.height);
      const pixels = context.getImageData(0, 0, sample.width, sample.height).data;
      const channels = [new Uint32Array(BINS), new Uint32Array(BINS), new Uint32Array(BINS)];
      for (let index = 0; index < pixels.length; index += 4) {
        channels[0][pixels[index] >> 2] += 1;
        channels[1][pixels[index + 1] >> 2] += 1;
        channels[2][pixels[index + 2] >> 2] += 1;
      }
      drawHistogram(canvas, channels);
    };
    image.src = source;

    return () => {
      disposed = true;
      image.onload = null;
    };
  }, [asset]);

  return (
    <canvas
      ref={canvasRef}
      className="histogram-canvas"
      width={BINS * 3}
      height={HEIGHT}
    />
  );
}

function drawHistogram(canvas: HTMLCanvasElement, channels: Uint32Array[]) {
  const context = canvas.getContext("2d");
  if (!context) return;
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.strokeStyle = "rgba(255,255,255,.055)";
  context.lineWidth = 1;
  for (let line = 1; line < 4; line += 1) {
    const y = (canvas.height * line) / 4;
    context.beginPath();
    context.moveTo(0, y);
    context.lineTo(canvas.width, y);
    context.stroke();
  }

  const max = Math.max(1, ...channels.flatMap((channel) => Array.from(channel)));
  const colors = ["rgba(255,105,120,.72)", "rgba(105,224,150,.7)", "rgba(120,145,255,.72)"];
  for (let channel = 0; channel < 3; channel += 1) {
    context.beginPath();
    for (let bin = 0; bin < BINS; bin += 1) {
      const x = (bin / (BINS - 1)) * canvas.width;
      const y = canvas.height - (channels[channel][bin] / max) * (canvas.height - 5);
      if (bin === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    context.strokeStyle = colors[channel];
    context.lineWidth = 1.8;
    context.stroke();
  }
}
