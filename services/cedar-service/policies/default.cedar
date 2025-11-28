// Default Cedar policies for Acton DX

// Allow authenticated users to read their own profile
permit (
    principal,
    action == Action::"read",
    resource
) when {
    principal == resource
};

// Allow admins to perform any action
permit (
    principal,
    action,
    resource
) when {
    principal.role == "admin"
};

// Deny all other requests by default (implicit)
