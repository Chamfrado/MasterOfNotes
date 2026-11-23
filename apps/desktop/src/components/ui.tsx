// Base components (SPEC-010 T-010-02). Native elements first: <dialog>, <select>, <progress>, <input type=range>.
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type HTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
  type Ref,
  type SelectHTMLAttributes,
} from "react";

const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(" ");

/** Turns anything an IPC call can reject with (AppError, Error, string) into a pt-BR message. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e && typeof e.message === "string") return e.message;
  return "Erro desconhecido.";
}

// ---------- Button ----------
type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
const buttonVariants: Record<ButtonVariant, string> = {
  primary: "bg-accent text-accent-fg hover:bg-accent-strong shadow-glow",
  secondary: "bg-surface-2 text-fg border border-line hover:bg-surface-3",
  ghost: "text-fg hover:bg-surface-2",
  danger: "bg-danger text-accent-fg hover:brightness-110",
};
interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  ref?: Ref<HTMLButtonElement>; // React 19: ref is a plain prop, forwarded by the spread
  variant?: ButtonVariant;
  size?: "md" | "lg";
}
export function Button({ variant = "secondary", size = "md", className, type = "button", ...props }: ButtonProps) {
  return (
    <button
      type={type}
      className={cx(
        "inline-flex min-h-12 min-w-12 cursor-pointer items-center justify-center gap-2 rounded-md font-semibold",
        "transition-[transform,opacity] duration-(--duration-fast) ease-out active:scale-[0.97]",
        "disabled:cursor-not-allowed disabled:opacity-50 disabled:active:scale-100",
        size === "lg" ? "px-7 text-lg" : "px-5",
        buttonVariants[variant],
        className,
      )}
      {...props}
    />
  );
}

// ---------- Field wrapper (label + hint/error wired with aria) ----------
function Field({
  id,
  label,
  hint,
  error,
  children,
}: {
  id: string;
  label: string;
  hint?: string;
  error?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-sm font-medium text-muted">
        {label}
      </label>
      {children}
      {error ? (
        <p id={`${id}-msg`} className="text-sm text-danger">
          {error}
        </p>
      ) : hint ? (
        <p id={`${id}-msg`} className="text-sm text-muted">
          {hint}
        </p>
      ) : null}
    </div>
  );
}
const controlClass =
  "min-h-12 w-full rounded-md border border-line bg-surface-2 px-4 text-fg placeholder:text-muted aria-invalid:border-danger";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  label: string;
  hint?: string;
  error?: string;
}
export function Input({ label, hint, error, className, id, ...props }: InputProps) {
  const autoId = useId();
  const inputId = id ?? autoId;
  return (
    <Field id={inputId} label={label} hint={hint} error={error}>
      <input
        id={inputId}
        aria-invalid={error ? true : undefined}
        aria-describedby={error || hint ? `${inputId}-msg` : undefined}
        className={cx(controlClass, className)}
        {...props}
      />
    </Field>
  );
}

interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label: string;
  hint?: string;
  options: { value: string; label: string }[];
}
export function Select({ label, hint, options, className, id, ...props }: SelectProps) {
  const autoId = useId();
  const selectId = id ?? autoId;
  return (
    <Field id={selectId} label={label} hint={hint}>
      <select
        id={selectId}
        aria-describedby={hint ? `${selectId}-msg` : undefined}
        className={cx(controlClass, "cursor-pointer", className)}
        {...props}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </Field>
  );
}

interface SliderProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "type" | "value" | "onChange"> {
  label: string;
  value: number;
  onChange: (value: number) => void;
  /** Visible value text, also announced (aria-valuetext). */
  format?: (value: number) => string;
}
export function Slider({ label, value, onChange, format = String, id, ...props }: SliderProps) {
  const autoId = useId();
  const sliderId = id ?? autoId;
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex justify-between text-sm">
        <label htmlFor={sliderId} className="font-medium text-muted">
          {label}
        </label>
        <output htmlFor={sliderId} className="tabular-nums">
          {format(value)}
        </output>
      </div>
      <input
        id={sliderId}
        type="range"
        value={value}
        aria-valuetext={format(value)}
        onChange={(e) => onChange(Number(e.currentTarget.value))}
        className="h-12 w-full cursor-pointer"
        {...props}
      />
    </div>
  );
}

// ---------- Card / Badge ----------
export function Card({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cx("rounded-lg border border-line bg-surface p-6 shadow-card", className)} {...props} />;
}

export type Tone = "neutral" | "accent" | "success" | "warning" | "danger" | "info";
const toneClass: Record<Tone, string> = {
  neutral: "text-muted border-line",
  accent: "text-accent border-accent/40",
  success: "text-success border-success/40",
  warning: "text-warning border-warning/40",
  danger: "text-danger border-danger/40",
  info: "text-info border-info/40",
};
export function Badge({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span className={cx("inline-flex items-center rounded-sm border px-2 py-0.5 text-sm font-semibold", toneClass[tone])}>
      {children}
    </span>
  );
}

// ---------- Progress ----------
/** Native <progress>; omit `value` for indeterminate. */
export function Progress({ label, value, max = 1 }: { label: string; value?: number; max?: number }) {
  return <progress aria-label={label} value={value} max={max} className="h-2 w-full" />;
}

// ---------- States ----------
export function LoadingState({ label = "Carregando…" }: { label?: string }) {
  return (
    <div className="flex flex-col items-center gap-4 py-16 text-muted">
      <div className="w-48">
        <Progress label={label} />
      </div>
      <p>{label}</p>
    </div>
  );
}

export function ErrorState({ error, onRetry, title = "Algo deu errado" }: { error: unknown; onRetry?: () => void; title?: string }) {
  return (
    <Card role="alert" className="flex max-w-xl flex-col items-start gap-3 border-danger/40">
      <h2 className="font-display text-xl font-semibold text-danger">{title}</h2>
      <p className="text-muted">{errorMessage(error)}</p>
      {onRetry && <Button onClick={onRetry}>Tentar de novo</Button>}
    </Card>
  );
}

// ---------- Dialog ----------
/** Native modal <dialog>: focus trap, Esc and inert background come from the platform. */
export function Dialog({
  open,
  onClose,
  title,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) d.showModal();
    if (!open && d.open) d.close();
  }, [open]);
  return (
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      onClose={onClose}
      className="m-auto w-[min(32rem,calc(100vw-2rem))] rounded-xl border border-line bg-surface p-8 text-fg shadow-card backdrop:bg-bg/80 open:animate-enter"
    >
      <h2 id={titleId} className="mb-6 font-display text-2xl font-semibold">
        {title}
      </h2>
      {children}
    </dialog>
  );
}

// ---------- Toast ----------
type ToastTone = "info" | "success" | "error";
interface ToastItem {
  id: number;
  tone: ToastTone;
  text: string;
}
const ToastContext = createContext<(text: string, tone?: ToastTone) => void>(() => {});
export const useToast = () => useContext(ToastContext);

const TOAST_MS = 6000;
export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const nextId = useRef(0);
  const dismiss = useCallback((id: number) => setItems((xs) => xs.filter((t) => t.id !== id)), []);
  const push = useCallback(
    (text: string, tone: ToastTone = "info") => {
      const id = nextId.current++;
      setItems((xs) => [...xs, { id, tone, text }]);
      setTimeout(() => dismiss(id), TOAST_MS);
    },
    [dismiss],
  );
  return (
    <ToastContext.Provider value={push}>
      {children}
      {/* Live regions exist before content arrives so screen readers announce it. */}
      <div className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-[min(26rem,calc(100vw-2rem))] flex-col gap-3">
        <div role="status" className="contents">
          {items.filter((t) => t.tone !== "error").map((t) => <ToastCard key={t.id} item={t} onDismiss={dismiss} />)}
        </div>
        <div role="alert" className="contents">
          {items.filter((t) => t.tone === "error").map((t) => <ToastCard key={t.id} item={t} onDismiss={dismiss} />)}
        </div>
      </div>
    </ToastContext.Provider>
  );
}
function ToastCard({ item, onDismiss }: { item: ToastItem; onDismiss: (id: number) => void }) {
  const bar = { info: "bg-info", success: "bg-success", error: "bg-danger" }[item.tone];
  return (
    <div className="pointer-events-auto flex animate-enter items-center gap-3 overflow-hidden rounded-md border border-line bg-surface-2 py-3 pr-2 pl-0 shadow-card">
      <span aria-hidden className={cx("w-1.5 self-stretch", bar)} />
      <p className="flex-1">{item.text}</p>
      <Button variant="ghost" aria-label="Fechar aviso" onClick={() => onDismiss(item.id)}>
        ✕
      </Button>
    </div>
  );
}

// ---------- NavBar ----------
interface NavItem<K extends string> {
  key: K;
  label: string;
}
export function NavBar<K extends string>({
  items,
  current,
  onNavigate,
}: {
  items: NavItem<K>[];
  current: K | null;
  onNavigate: (key: K) => void;
}) {
  return (
    <nav aria-label="Principal" className="flex items-center gap-6 border-b border-line/60 px-8 py-3">
      <span className="bg-(image:--gradient-accent) bg-clip-text font-display text-2xl font-bold text-transparent">
        Master Of Notes
      </span>
      <ul className="flex flex-1 gap-1">
        {items.map((it) => (
          <li key={it.key}>
            <button
              type="button"
              aria-current={it.key === current ? "page" : undefined}
              onClick={() => onNavigate(it.key)}
              className="min-h-12 cursor-pointer rounded-md px-4 font-medium text-muted transition-opacity duration-(--duration-fast) hover:text-fg aria-[current=page]:bg-surface-2 aria-[current=page]:text-fg"
            >
              {it.label}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
