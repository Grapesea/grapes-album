import type { Locale } from "./i18n";

const dimensionNames: Record<
  string,
  { zh: string; en: string }
> = {
  date: { zh: "日期", en: "Date" },
  time: { zh: "时段", en: "Time" },
  weather: { zh: "天气", en: "Weather" },
  location: { zh: "地点", en: "Location" },
  type: { zh: "类型", en: "Type" },
  camera: { zh: "相机", en: "Camera" },
  lens: { zh: "镜头", en: "Lens" },
  tone: { zh: "色调", en: "Tone" },
  custom: { zh: "自定义", en: "Custom" },
  tags: { zh: "标签", en: "Tags" },
};

const valueNames: Record<string, { zh: string; en: string }> = {
  morning: { zh: "晨曦", en: "Morning" },
  day: { zh: "白天", en: "Day" },
  "golden-hour": { zh: "黄金时刻", en: "Golden Hour" },
  "blue-hour": { zh: "蓝调时刻", en: "Blue Hour" },
  night: { zh: "夜晚", en: "Night" },
  sunny: { zh: "晴", en: "Sunny" },
  cloudy: { zh: "多云", en: "Cloudy" },
  overcast: { zh: "阴", en: "Overcast" },
  rain: { zh: "雨", en: "Rain" },
  snow: { zh: "雪", en: "Snow" },
  fog: { zh: "雾", en: "Fog" },
  landscape: { zh: "风光", en: "Landscape" },
  portrait: { zh: "人像", en: "Portrait" },
  documentary: { zh: "纪实", en: "Documentary" },
  street: { zh: "街头", en: "Street" },
  architecture: { zh: "建筑", en: "Architecture" },
  "still-life": { zh: "静物", en: "Still Life" },
  wildlife: { zh: "生态", en: "Wildlife" },
  warm: { zh: "暖色", en: "Warm" },
  cool: { zh: "冷色", en: "Cool" },
  neutral: { zh: "中性", en: "Neutral" },
  "high-saturation": { zh: "高饱和", en: "High Saturation" },
  "low-saturation": { zh: "低饱和", en: "Low Saturation" },
  cinematic: { zh: "电影感", en: "Cinematic" },
};

export function dimensionDisplayName(
  identifier: string,
  fallback: string,
  locale: Locale,
) {
  const key = identifier.split(":").at(-1) ?? identifier;
  return dimensionNames[key]?.[locale] ?? fallback;
}

export function classificationValueDisplayName(
  valueId: string,
  fallback: string,
  locale: Locale,
) {
  const marker = ":value:";
  const key = valueId.includes(marker)
    ? valueId.split(marker).at(-1) ?? fallback
    : valueId.split(":").at(-1) ?? fallback;
  const known = valueNames[key];
  if (known) return known[locale];
  const byEnglishName = Object.values(valueNames).find(
    (item) => item.en.toLocaleLowerCase() === fallback.toLocaleLowerCase(),
  );
  return byEnglishName?.[locale] ?? fallback;
}
