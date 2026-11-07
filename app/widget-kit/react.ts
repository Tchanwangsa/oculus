/**
 * `react` for widgets (the import map points it here). React ships as
 * CommonJS, so every name is re-exported explicitly: `export *` from a CJS
 * module leaves an ES consumer's named imports to the bundler's guesswork.
 * `tests/widget-kit/kit.test.tsx` holds this list to React's own keys.
 */
import * as React from "react";

export default React;
export const {
  Activity,
  Children,
  Component,
  Fragment,
  Profiler,
  PureComponent,
  StrictMode,
  Suspense,
  cache,
  cloneElement,
  createContext,
  createElement,
  createRef,
  forwardRef,
  isValidElement,
  lazy,
  memo,
  startTransition,
  use,
  useActionState,
  useCallback,
  useContext,
  useDebugValue,
  useDeferredValue,
  useEffect,
  useEffectEvent,
  useId,
  useImperativeHandle,
  useInsertionEffect,
  useLayoutEffect,
  useMemo,
  useOptimistic,
  useReducer,
  useRef,
  useState,
  useSyncExternalStore,
  useTransition,
  version,
} = React as typeof React & { Activity: unknown; useEffectEvent: unknown };
