/** `locale` on the Gregorian calendar. A Thai region (en-TH, th-TH) defaults
 *  to the Buddhist one, which prints "October 2569 BE". */
export function gregorian(locale: string): string {
  try {
    return new Intl.Locale(locale, { calendar: "gregory" }).toString();
  } catch {
    return locale;
  }
}

function systemLocale(): string {
  return new Intl.DateTimeFormat().resolvedOptions().locale;
}

/** The locale every date is formatted in: the system's on the Gregorian
 *  calendar, unless a test pins one so its expected strings hold on any
 *  machine. */
export let displayLocale: string = gregorian(systemLocale());

export function pinDisplayLocale(locale: string | undefined): void {
  displayLocale = gregorian(locale ?? systemLocale());
}
