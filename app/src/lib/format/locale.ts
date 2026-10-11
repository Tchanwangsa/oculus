/** The locale every date is formatted in: the system's (`undefined`), unless
 *  a test pins one so its expected strings hold on any machine. */
export let displayLocale: string | undefined = undefined;

export function pinDisplayLocale(locale: string | undefined): void {
  displayLocale = locale;
}
