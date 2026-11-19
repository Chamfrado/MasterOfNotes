import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Button, Card, Dialog, ErrorState, Input, NavBar, Progress, Select, Slider, ToastProvider, errorMessage, useToast } from "./ui";

describe("Button", () => {
  it("is a real button reachable by Tab and activated by Enter/Space", async () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Cantar</Button>);
    const btn = screen.getByRole("button", { name: "Cantar" });
    expect(btn).toHaveAttribute("type", "button");
    await userEvent.tab();
    expect(btn).toHaveFocus();
    await userEvent.keyboard("{Enter}");
    await userEvent.keyboard(" ");
    expect(onClick).toHaveBeenCalledTimes(2);
  });

  it("does not fire when disabled", async () => {
    const onClick = vi.fn();
    render(<Button disabled onClick={onClick}>X</Button>);
    await userEvent.click(screen.getByRole("button"));
    expect(onClick).not.toHaveBeenCalled();
  });
});

describe("Input", () => {
  it("is labelled and exposes error via aria-invalid/aria-describedby", () => {
    render(<Input label="Título" error="Obrigatório" />);
    const input = screen.getByRole("textbox", { name: "Título" });
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("Obrigatório");
  });

  it("uses hint as description when valid", () => {
    render(<Input label="Artista" hint="Opcional" />);
    const input = screen.getByRole("textbox", { name: "Artista" });
    expect(input).not.toHaveAttribute("aria-invalid");
    expect(input).toHaveAccessibleDescription("Opcional");
  });
});

describe("Select", () => {
  it("is a labelled combobox that changes by keyboard", async () => {
    const onChange = vi.fn();
    render(
      <Select
        label="Dificuldade"
        defaultValue="easy"
        onChange={(e) => onChange(e.currentTarget.value)}
        options={[
          { value: "easy", label: "Fácil" },
          { value: "hard", label: "Difícil" },
        ]}
      />,
    );
    const select = screen.getByRole("combobox", { name: "Dificuldade" });
    await userEvent.selectOptions(select, "hard");
    expect(onChange).toHaveBeenCalledWith("hard");
  });
});

describe("Slider", () => {
  // Arrow keys on a native range are browser behaviour (jsdom does not emulate them); we check the change wiring.
  it("is a labelled slider with value text that reports changes", () => {
    function Harness() {
      const [v, setV] = useState(50);
      return <Slider label="Volume" min={0} max={100} value={v} onChange={setV} format={(n) => `${n}%`} />;
    }
    render(<Harness />);
    const slider = screen.getByRole("slider", { name: "Volume" });
    expect(slider).toHaveAttribute("aria-valuetext", "50%");
    fireEvent.change(slider, { target: { value: "51" } });
    expect(slider).toHaveAttribute("aria-valuetext", "51%");
  });
});

describe("Progress", () => {
  it("exposes a labelled progressbar", () => {
    render(<Progress label="Varrendo" value={0.5} />);
    expect(screen.getByRole("progressbar", { name: "Varrendo" })).toBeInTheDocument();
  });
});

describe("Card", () => {
  it("passes through role and content", () => {
    render(<Card role="region" aria-label="Resumo">oi</Card>);
    expect(screen.getByRole("region", { name: "Resumo" })).toHaveTextContent("oi");
  });
});

describe("Dialog", () => {
  it("opens as a titled dialog and reports close", async () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <Dialog open={false} onClose={onClose} title="Editar música">
        <button>Salvar</button>
      </Dialog>,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    rerender(
      <Dialog open onClose={onClose} title="Editar música">
        <button>Salvar</button>
      </Dialog>,
    );
    expect(screen.getByRole("dialog", { name: "Editar música" })).toBeInTheDocument();
    rerender(
      <Dialog open={false} onClose={onClose} title="Editar música">
        <button>Salvar</button>
      </Dialog>,
    );
    expect(onClose).toHaveBeenCalled();
  });
});

describe("Toast", () => {
  function Trigger({ tone }: { tone: "info" | "error" }) {
    const toast = useToast();
    return <Button onClick={() => toast("Pasta indexada", tone)}>disparar</Button>;
  }
  it("announces info in a status region and can be dismissed", async () => {
    render(
      <ToastProvider>
        <Trigger tone="info" />
      </ToastProvider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "disparar" }));
    expect(screen.getByRole("status")).toHaveTextContent("Pasta indexada");
    await userEvent.click(screen.getByRole("button", { name: "Fechar aviso" }));
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
  });
  it("announces errors in an alert region", async () => {
    render(
      <ToastProvider>
        <Trigger tone="error" />
      </ToastProvider>,
    );
    await userEvent.click(screen.getByRole("button", { name: "disparar" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Pasta indexada");
  });
});

describe("NavBar", () => {
  it("marks the current page and navigates", async () => {
    const onNavigate = vi.fn();
    render(
      <NavBar
        items={[
          { key: "home", label: "Início" },
          { key: "library", label: "Biblioteca" },
        ]}
        current="home"
        onNavigate={onNavigate}
      />,
    );
    expect(screen.getByRole("navigation", { name: "Principal" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Início" })).toHaveAttribute("aria-current", "page");
    await userEvent.click(screen.getByRole("button", { name: "Biblioteca" }));
    expect(onNavigate).toHaveBeenCalledWith("library");
  });
});

describe("ErrorState / errorMessage", () => {
  it("shows the AppError message in an alert with retry", async () => {
    const retry = vi.fn();
    render(<ErrorState error={{ code: "db", message: "Banco indisponível." }} onRetry={retry} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Banco indisponível.");
    await userEvent.click(screen.getByRole("button", { name: "Tentar de novo" }));
    expect(retry).toHaveBeenCalled();
  });
  it("handles strings, Errors and unknowns", () => {
    expect(errorMessage("command list_songs not found")).toBe("command list_songs not found");
    expect(errorMessage(new Error("boom"))).toBe("boom");
    expect(errorMessage(42)).toBe("Erro desconhecido.");
  });
});
