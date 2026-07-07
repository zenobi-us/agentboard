import type { ReactNode } from "react";
import classNames from "classnames";
import { HeroPanels } from "./HeroPanels";

export type AboveTheFoldPanelsProps = {
  readonly left: ReactNode;
  readonly right: ReactNode;
};

export function AboveTheFold() {
  return (
    <HeroPanels
      left={(
        <>
          <h1
            className={classNames(
              "max-w-3xl font-semibold leading-none tracking-tighter",
              "text-[clamp(1.5rem,8vw,3rem)] xl:text-[clamp(3rem,4vw,6rem)]",
            )}
          >
            Trackers everywhere. Work starts nowhere.
          </h1>
          <p className="ml-auto text-xs font-medium uppercase tracking-[0.16em] text-rp-subtle">
            Before
          </p>
        </>
      )}
      right={(
        <>
          <h2
            className={classNames(
              "max-w-3xl font-semibold leading-none tracking-tighter",
              "text-[clamp(1.5rem,8vw,3rem)] xl:text-[clamp(3rem,4vw,6rem)]",
            )}
          >
            Collect once. Run repeatable actions.
          </h2>
          <p className="self-end text-xs font-medium uppercase tracking-[0.16em] text-rp-overlay lg:self-auto">
            After
          </p>
        </>
      )}
    />
  );
}
