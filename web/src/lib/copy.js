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

export const LOCKS = [
  {
    value: "game",
    label: "Game",
    detail:
      "Lineups lock for a game. A change made during a game goes live for the next game. ULMG is an example.",
  },
  {
    value: "week",
    label: "Week",
    detail:
      "Lineups lock for a week. Managers may keep editing during the week. The change goes live next week. NPL is an example.",
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
