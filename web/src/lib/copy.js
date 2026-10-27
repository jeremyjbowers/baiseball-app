export const POSITIONS = [
  { value: "C", label: "Catcher" },
  { value: "1B", label: "First base" },
  { value: "2B", label: "Second base" },
  { value: "3B", label: "Third base" },
  { value: "SS", label: "Shortstop" },
  { value: "LF", label: "Left field" },
  { value: "CF", label: "Center field" },
  { value: "RF", label: "Right field" },
  { value: "DH", label: "Designated hitter" },
];

export const TYPES = [
  {
    value: "ulmg",
    label: "ULMG",
    lock: "game",
    detail: "One game at a time. A season behind. Lineups lock by game.",
  },
  {
    value: "npl",
    label: "NPL",
    lock: "week",
    detail: "Simulate a week. Mix current and past stats. Lineups lock by week.",
  },
];

export function typeDetail(value) {
  return TYPES.find((type) => type.value === value)?.detail ?? "Choose ULMG or NPL.";
}

export function typeLabel(value) {
  return TYPES.find((type) => type.value === value)?.label ?? "League type not set";
}

export function countNote(seasonLength, divisionGames, outsideGames) {
  const season = String(seasonLength ?? "").trim();
  const inside = String(divisionGames ?? "").trim();
  const outside = String(outsideGames ?? "").trim();
  if (!season || !inside || !outside) {
    return {
      ok: false,
      text: "The games inside the division and the games outside it have to add up to the season length.",
    };
  }
  if (!/^\d+$/.test(season) || !/^\d+$/.test(inside) || !/^\d+$/.test(outside)) {
    return { ok: false, text: "Use whole numbers of games." };
  }
  const seasonN = Number(season);
  const insideN = Number(inside);
  const outsideN = Number(outside);
  if (seasonN < 1) return { ok: false, text: "Season length must be at least 1 game." };
  if (seasonN > 366 || insideN > 366 || outsideN > 366) {
    return { ok: false, text: "A season here is 366 games or fewer." };
  }
  if (insideN + outsideN !== seasonN) {
    return {
      ok: false,
      text: `${insideN} inside and ${outsideN} outside add up to ${insideN + outsideN}, not the season length of ${seasonN}.`,
    };
  }
  return {
    ok: true,
    text: `${insideN} inside the division and ${outsideN} outside it. ${seasonN} games for each team.`,
    season: seasonN,
    inside: insideN,
    outside: outsideN,
  };
}

export const LOCKS = [
  {
    value: "game",
    label: "Game",
    detail: "ULMG locks lineups by game. One game at a time, a season behind.",
  },
  {
    value: "week",
    label: "Week",
    detail: "NPL locks lineups by week. The league simulates a week and mixes current and past stats.",
  },
  {
    value: "half",
    label: "Half",
    detail: "Lineups lock for a half. A half ends on game 81.",
  },
  {
    value: "series",
    label: "Series",
    detail: "Lineups lock for a series. A series is consecutive games against the same team.",
  },
];

export function lockDetail(value) {
  return LOCKS.find((lock) => lock.value === value)?.detail ?? value;
}

export function positionLabel(value) {
  return POSITIONS.find((position) => position.value === value)?.label ?? value;
}

export function emptyCard() {
  return Array.from({ length: 9 }, (_, index) => ({
    battingOrder: index + 1,
    playerName: "",
    position: "",
  }));
}

export function formatWhen(iso) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date);
}
