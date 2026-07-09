import classnames from "classnames";
import type { PropsWithChildren, HTMLAttributes, ReactNode } from "react";

export function Logo(
  props: PropsWithChildren<
    HTMLAttributes<SVGSVGElement> & {
      suffix?: ReactNode;
    }
  >,
) {
  return (
    <div
      className={classnames(
        "flex items-center gap-2 text-4xl font-bold",
        props.className,
      )}
    >
      <h1><span className="material-symbols-outline">developer_board</span> AgentBoard{props.suffix}</h1>
    </div>
  );
}
