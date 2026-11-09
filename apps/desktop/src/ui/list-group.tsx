import { Children, cloneElement, isValidElement } from "react";
import type { ReactElement, ReactNode } from "react";

import { cn } from "./cn";

/** Inset-grouped list on `surface`: rows separated by inset hairlines. */
export function ListGroup({
  title,
  footer,
  children,
}: {
  title?: string | undefined;
  footer?: string | undefined;
  children?: ReactNode;
}): ReactElement {
  const items = Children.toArray(children).map((child, index): ReactNode =>
    isValidElement<ListRowProps>(child) && child.type === ListRow
      ? cloneElement(child, { separator: index > 0 })
      : child,
  );
  return (
    <div className="mt-5 first:mt-0">
      {title !== undefined && <p className="text-muted px-4 pb-1.5 text-xs">{title}</p>}
      <div className="bg-surface rounded-group overflow-hidden">{items}</div>
      {footer !== undefined && <p className="text-muted px-4 pt-1.5 text-xs">{footer}</p>}
    </div>
  );
}

export type ListRowProps = {
  label: string;
  description?: string | undefined;
  /** Hairline injected by ListGroup. */
  separator?: boolean | undefined;
  onClick?: (() => void) | undefined;
  children?: ReactNode;
};

export function ListRow({
  label,
  description,
  separator,
  onClick,
  children,
}: ListRowProps): ReactElement {
  const body = (
    <>
      {separator === true && (
        <div aria-hidden="true" className="bg-line absolute top-0 right-0 left-4 h-px" />
      )}
      <div className="min-w-0 flex-1">
        <p className="text-ink truncate text-base">{label}</p>
        {description !== undefined && (
          <p className="text-muted truncate text-xs select-text">{description}</p>
        )}
      </div>
      {children}
    </>
  );
  const rowClass = cn(
    "relative flex min-h-13 w-full items-center gap-3 px-4 py-2.5 text-left",
    onClick !== undefined && "hover:bg-hover cursor-default",
  );
  if (onClick !== undefined) {
    return (
      <button type="button" className={rowClass} onClick={onClick}>
        {body}
      </button>
    );
  }
  return <div className={rowClass}>{body}</div>;
}
