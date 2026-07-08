import type { HTMLAttributes, PropsWithChildren } from 'react';
import '../app.css';

export function Site(props: PropsWithChildren<HTMLAttributes<HTMLElement>>) {
  const { children, ...rest } = props;
  return (
    <main className="flex min-h-screen flex-col  bg-rp-base text-rp-text" {...rest}>
      {children}
    </main>
  )
}
