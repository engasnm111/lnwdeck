import { Link, NavLink, Outlet } from "react-router-dom";

const navItems = [
  { to: "/", label: "Overview" },
  { to: "/providers", label: "Providers" },
  { to: "/analytics", label: "Analytics" },
  { to: "/costs", label: "Costs" },
  { to: "/budgets", label: "Budgets" },
  { to: "/models", label: "Models" },
  { to: "/alerts", label: "Alerts" },
];

export function AppShell() {
  return (
    <div style={{ display: "flex", minHeight: "100vh" }}>
      <nav
        role="navigation"
        aria-label="Main navigation"
        style={{
          width: 220,
          borderRight: "1px solid #e0e0e0",
          padding: "1rem",
        }}
      >
        <h1 style={{ fontSize: "1.25rem", marginBottom: "1.5rem" }}>
          <Link to="/" style={{ textDecoration: "none", color: "inherit" }}>
            inwdeck
          </Link>
        </h1>
        <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
          {navItems.map((item) => (
            <li key={item.to} style={{ marginBottom: "0.5rem" }}>
              <NavLink
                to={item.to}
                style={({ isActive }) => ({
                  display: "block",
                  padding: "0.5rem 0.75rem",
                  borderRadius: 6,
                  textDecoration: "none",
                  color: isActive ? "#fff" : "#333",
                  background: isActive ? "#3b82f6" : "transparent",
                })}
              >
                {item.label}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>
      <main style={{ flex: 1, padding: "2rem" }}>
        <Outlet />
      </main>
    </div>
  );
}
